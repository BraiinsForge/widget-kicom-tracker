//! Small, asynchronous, URL-keyed portrait cache. Call `get` only in render scope.
use bmc_wasm_sdk::*;
use std::cell::RefCell;

struct Entry {
    url: String,
    tag: String,
    bitmap: Option<BitmapId>,
    request: Option<FetchRequestId>,
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
            let tag = format!("kicom.portrait.{}", url);
            entries.push(Entry {
                bitmap: register_bitmap_from_cache(&tag),
                tag,
                url,
                request: None,
                job: None,
                retry_at: 0,
            });
            entries.len() - 1
        };
        let entry = &mut entries[index];
        let now = SystemTime::now().unix_secs;
        if entry.job.is_some_and(|job| !image_decode_is_pending(job)) {
            entry.job = None;
        }
        if entry.bitmap.is_none()
            && entry.request.is_none()
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
            // Host performs the JPEG decode and square crop off the render thread.
            entry.job = set_bitmap_fit(
                &entry.tag,
                response.body(),
                128,
                128,
                true,
                entry.url.as_bytes(),
                on_decoded,
            );
        }
    });
    request_frame();
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
