//! Small, asynchronous, URL-keyed portrait cache. Call `get` only in render scope.
use bmc_wasm_sdk::*;
use std::cell::RefCell;

struct Entry {
    url: String,
    tag: String,
    bitmap: Option<BitmapId>,
    request: Option<FetchRequestId>,
    body: Option<Vec<u8>>,
    job: Option<ImageJobId>,
    retry_at: i64,
}

thread_local! {
    static IMAGES: RefCell<Vec<Entry>> = const { RefCell::new(Vec::new()) };
}

pub fn get(path: &str) -> Option<BitmapId> {
    let url = crate::model::avatar_url(path)?;
    IMAGES.with_borrow_mut(|entries| {
        let index = if let Some(i) = entries.iter().position(|e| e.url == url) {
            i
        } else {
            // Bound decoded GPU/host memory, even if the upstream catalog grows.
            if entries.len() >= 32 {
                return None;
            }
            let tag = cache_tag(&url);
            entries.push(Entry {
                bitmap: register_bitmap_from_cache(&tag),
                tag,
                url,
                request: None,
                body: None,
                job: None,
                retry_at: 0,
            });
            entries.len() - 1
        };
        decode_next(entries);
        let entry = &mut entries[index];
        let now = SystemTime::now().unix_secs;
        if entry.bitmap.is_none()
            && entry.request.is_none()
            && entry.body.is_none()
            && entry.job.is_none()
            && now >= entry.retry_at
        {
            entry.retry_at = now + 300;
            entry.request = FetchRequest::get(&entry.url)
                .timeout(std::time::Duration::from_secs(15))
                .send(on_photo);
        }
        entry.bitmap
    })
}

fn on_photo(response: &FetchResponse) {
    IMAGES.with_borrow_mut(|entries| {
        let Some(entry) = entries
            .iter_mut()
            .find(|e| e.request == Some(response.request_id))
        else {
            return;
        };
        entry.request = None;
        if response.ok() && !response.body().is_empty() && response.body().len() <= 512_000 {
            entry.body = Some(response.body().to_vec());
        }
        decode_next(entries);
    });
    request_frame();
}

/// Start the oldest waiting decode unless one is already running.
/// The host decodes one image per widget at a time and refuses the rest,
/// so a refused body stays queued for the next frame.
fn decode_next(entries: &mut [Entry]) {
    for entry in entries.iter_mut() {
        if entry.job.is_some_and(|job| !image_decode_is_pending(job)) {
            entry.job = None;
        }
    }
    if entries.iter().any(|e| e.job.is_some()) {
        return;
    }
    let Some(entry) = entries.iter_mut().find(|e| e.body.is_some()) else {
        return;
    };
    let body = entry
        .body
        .as_deref()
        .expect("BUG: the entry was found by its body");
    // Host performs the JPEG decode and square crop off the render thread.
    entry.job = set_bitmap_fit(
        &entry.tag,
        body,
        128,
        128,
        true,
        entry.url.as_bytes(),
        on_decoded,
    );
    if entry.job.is_some() {
        entry.body = None;
    }
}

/// The host refuses a cache key that holds a path separator, and `avatar_url`
/// already limits the file name to characters a key may hold.
fn cache_tag(url: &str) -> String {
    let file = url
        .rsplit('/')
        .next()
        .expect("BUG: rsplit yields at least one piece");
    format!("kicom.portrait.{file}")
}

#[cfg(test)]
mod tests {
    use super::cache_tag;

    #[test]
    fn cache_tag_has_no_path_separator() {
        let url =
            crate::model::avatar_url("assets/avatars/Kicom21.jpg").expect("a valid avatar path");
        let tag = cache_tag(&url);
        assert!(!tag.contains('/'), "the host refuses {tag} as a cache key");
        assert_eq!(tag, "kicom.portrait.Kicom21.jpg");
    }
}

fn on_decoded(job: ImageJobId, bitmap: Option<BitmapId>) {
    IMAGES.with_borrow_mut(|entries| {
        if let Some(entry) = entries.iter_mut().find(|e| e.job == Some(job)) {
            entry.job = None;
            entry.bitmap = bitmap;
        }
    });
    request_frame();
}
