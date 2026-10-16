//! Bounded, asynchronous decoding of the bundled NASA diffuse sky derivative.
use image::{ImageFormat, ImageReader, Limits};
use std::{io::Cursor, sync::Arc};
pub const CREDIT: &str = "NASA/Goddard Space Flight Center Scientific Visualization Studio; Gaia DR2: ESA/Gaia/DPAC; visualization: Ernie Wright";
pub const SOURCE_URL: &str = "https://svs.gsfc.nasa.gov/4851/";
const ENCODED: &[u8] = include_bytes!("../../../assets/sky/milkyway-2020-4k-sdr.jpg");
const MAX_ENCODED_BYTES: usize = 8 * 1024 * 1024;
const MAX_DECODE_BYTES: u64 = 64 * 1024 * 1024;
pub const WIDTH_PX: u32 = 4096;
pub const HEIGHT_PX: u32 = 2048;
pub struct SkyImage {
    pixels: image::RgbImage,
}
impl SkyImage {
    pub fn pixels(&self) -> &image::RgbImage {
        &self.pixels
    }
    /// Accepts only the supported 4K RGB derivative; no arbitrary giant textures.
    pub fn decode(encoded: &[u8]) -> Result<Self, crate::Error> {
        if encoded.len() > MAX_ENCODED_BYTES {
            return Err("Celestial image exceeds the encoded limit".into());
        }
        let mut limits = Limits::default();
        limits.max_image_width = Some(WIDTH_PX);
        limits.max_image_height = Some(HEIGHT_PX);
        limits.max_alloc = Some(MAX_DECODE_BYTES);
        let mut reader = ImageReader::with_format(Cursor::new(encoded), ImageFormat::Jpeg);
        reader.limits(limits);
        let pixels = reader.decode()?.into_rgb8();
        if pixels.dimensions() != (WIDTH_PX, HEIGHT_PX) {
            return Err("Celestial image must be the supported 4096×2048 map".into());
        }
        Ok(Self { pixels })
    }
}
#[derive(Default)]
pub struct SkyService {
    started: bool,
    pending: Option<tokio::sync::oneshot::Receiver<Result<Arc<SkyImage>, crate::Error>>>,
    pub image: Option<Arc<SkyImage>>,
    pub error: Option<crate::Error>,
}
impl SkyService {
    pub fn update(
        &mut self,
        runtime: &tokio::runtime::Handle,
        enabled: bool,
        repaint: impl FnOnce() + Send + 'static,
    ) {
        if let Some(receiver) = &mut self.pending {
            match receiver.try_recv() {
                Ok(result) => {
                    self.pending = None;
                    match result {
                        Ok(image) => self.image = Some(image),
                        Err(error) => self.error = Some(error),
                    }
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
                    self.pending = None;
                    self.error =
                        Some("Celestial decoder stopped before returning its result".into());
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {}
            }
        }
        if enabled && !self.started {
            self.started = true;
            let (send, receive) = tokio::sync::oneshot::channel();
            self.pending = Some(receive);
            runtime.spawn(async move {
                let result = crate::cpu::run(crate::cpu::Service::Tiles, || {
                    SkyImage::decode(ENCODED).map(Arc::new)
                })
                .await;
                let _ = send.send(result);
                repaint();
            });
        }
    }
    pub fn is_loading(&self) -> bool {
        self.pending.is_some()
    }
    pub fn retry(&mut self) {
        if self.pending.is_none() {
            self.started = false;
            self.error = None;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bundled_map_is_bounded_and_nonuniform_and_bad_data_fails() {
        let image = SkyImage::decode(ENCODED).unwrap();
        assert_eq!(image.pixels().dimensions(), (WIDTH_PX, HEIGHT_PX));
        assert_eq!(image.pixels().as_raw().len(), 24 * 1024 * 1024);
        assert!(
            image
                .pixels()
                .pixels()
                .any(|pixel| pixel.0 != image.pixels().get_pixel(0, 0).0)
        );
        assert!(SkyImage::decode(b"bad JPEG").is_err());
        assert!(SkyImage::decode(&vec![0; MAX_ENCODED_BYTES + 1]).is_err());
    }
    #[test]
    fn optional_async_decode_wakes_without_a_network_service() {
        let runtime = crate::runtime::ServiceRuntime::new().unwrap();
        let mut service = SkyService::default();
        service.update(runtime.handle(), false, || panic!("disabled load"));
        assert!(!service.is_loading());
        let (send, receive) = std::sync::mpsc::channel();
        service.update(runtime.handle(), true, move || {
            send.send(()).unwrap();
        });
        assert!(service.is_loading());
        receive
            .recv_timeout(std::time::Duration::from_secs(10))
            .unwrap();
        service.update(runtime.handle(), true, || panic!("duplicate load"));
        assert!(service.image.is_some());
        assert!(!service.is_loading());
        assert!(service.error.is_none());
    }
}
