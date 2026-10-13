//! Stream a feature collection, retaining projected data rather than the JSON tree.
use serde::de::{DeserializeSeed, IgnoredAny, MapAccess, SeqAccess, Visitor};
use std::{
    fmt,
    io::BufReader,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
use tokio::io::AsyncWriteExt;

struct Collection<'a, T, F> {
    out: &'a mut T,
    consume: &'a F,
}
impl<'de, T, F: Fn(&serde_json::Value, &mut T)> DeserializeSeed<'de> for Collection<'_, T, F> {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_map(self)
    }
}
impl<'de, T, F: Fn(&serde_json::Value, &mut T)> Visitor<'de> for Collection<'_, T, F> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a GeoJSON feature collection")
    }
    fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<(), M::Error> {
        let mut found = false;
        while let Some(key) = map.next_key::<String>()? {
            if key == "features" {
                map.next_value_seed(Features {
                    out: self.out,
                    consume: self.consume,
                })?;
                found = true;
            } else {
                map.next_value::<IgnoredAny>()?;
            }
        }
        if !found {
            return Err(serde::de::Error::custom("arquivo sem 'features'"));
        }
        Ok(())
    }
}
struct Features<'a, T, F> {
    out: &'a mut T,
    consume: &'a F,
}
impl<'de, T, F: Fn(&serde_json::Value, &mut T)> DeserializeSeed<'de> for Features<'_, T, F> {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_seq(self)
    }
}
impl<'de, T, F: Fn(&serde_json::Value, &mut T)> Visitor<'de> for Features<'_, T, F> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a feature array")
    }
    fn visit_seq<S: SeqAccess<'de>>(self, mut seq: S) -> Result<(), S::Error> {
        while let Some(feature) = seq.next_element::<serde_json::Value>()? {
            (self.consume)(&feature, self.out);
        }
        Ok(())
    }
}
fn project<T: Default, F: Fn(&serde_json::Value, &mut T)>(
    reader: impl std::io::Read,
    consume: F,
) -> Result<T, crate::Error> {
    let mut out = T::default();
    let mut deserializer = serde_json::Deserializer::from_reader(reader);
    Collection {
        out: &mut out,
        consume: &consume,
    }
    .deserialize(&mut deserializer)?;
    deserializer.end()?;
    Ok(out)
}
async fn read<T: Default + Send + 'static, F: Fn(&serde_json::Value, &mut T) + Send + 'static>(
    path: PathBuf,
    consume: F,
) -> Result<T, crate::Error> {
    tokio::task::spawn_blocking(move || {
        project(BufReader::new(std::fs::File::open(path)?), consume)
    })
    .await?
}
struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
async fn download(url: &str, path: &std::path::Path) -> Result<(), crate::Error> {
    let mut response = crate::http::client(crate::http::Policy::Boundary)?
        .get(url)
        .send()
        .await?
        .error_for_status()?;
    let mut file = tokio::fs::File::create(path).await?;
    let mut total = 0;
    while let Some(chunk) = response.chunk().await? {
        total += chunk.len() as u64;
        if total > crate::http::MAX_JSON_RESPONSE_BYTES {
            return Err("boundary response exceeds read limit".into());
        }
        file.write_all(&chunk).await?;
    }
    file.flush().await?;
    Ok(())
}
pub(crate) async fn fetch<T, F>(url: &str, consume: F) -> Result<T, crate::Error>
where
    T: Default + Send + 'static,
    F: Fn(&serde_json::Value, &mut T) + Clone + Send + 'static,
{
    // Bound concurrent downloads and projection, including readers of cached files.
    static PERMITS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);
    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);
    let _permit = PERMITS
        .acquire()
        .await
        .expect("feature semaphore remains open");
    let path = crate::http::geo_cache_path(url);
    if let Ok(out) = read(path.clone(), consume.clone()).await {
        return Ok(out);
    }
    let cache_writable = match path.parent() {
        Some(dir) => tokio::fs::create_dir_all(dir).await.is_ok(),
        None => false,
    };
    let mut last = None;
    for _ in 0..crate::http::JSON_REQUEST_ATTEMPTS {
        let name = format!(
            "earthcraft-download-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        );
        let temporary = Temporary(if cache_writable {
            path.with_extension(&name)
        } else {
            std::env::temp_dir().join(name)
        });
        match download(url, &temporary.0).await {
            Ok(()) => match read(temporary.0.clone(), consume.clone()).await {
                Ok(out) => {
                    if cache_writable {
                        let _ = tokio::fs::rename(&temporary.0, &path).await;
                    }
                    return Ok(out);
                }
                Err(error) => last = Some(error),
            },
            Err(error) => last = Some(error),
        }
    }
    Err(last.expect("at least one feature request attempt"))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn streaming_projection_skips_metadata_and_preserves_order() {
        let json=br#"{"type":"FeatureCollection","metadata":{"unused":[1,2,3]},"features":[{"id":1},{"id":2}]}"#;
        let ids: Vec<u64> = project(json.as_slice(), |value, out: &mut Vec<u64>| {
            out.push(value["id"].as_u64().unwrap())
        })
        .unwrap();
        assert_eq!(ids, vec![1, 2]);
        assert!(project::<Vec<u64>, _>(b"{}".as_slice(), |_, _| {}).is_err());
        assert!(project::<Vec<u64>, _>(b"{\"features\":[{}]".as_slice(), |_, _| {}).is_err());
    }
}
