use bevy_asset::{io::Writer, saver::SavedAsset, AssetPath, AsyncWriteExt};

use super::{CompressedImageSaverError, CompressedImageSaverSettings};
use crate::{Image, ImageFormat, ImageFormatSetting, ImageLoaderSettings};

use basis_universal::{
    BasisTextureFormat, ColorSpace, Compressor, CompressorParams, UASTC_QUALITY_DEFAULT,
};

#[derive(Default)]
pub struct CompressedImageSaverUniversal;

impl CompressedImageSaverUniversal {
    pub async fn save(
        &self,
        writer: &mut Writer,
        image: SavedAsset<'_, '_, Image>,
        settings: &CompressedImageSaverSettings,
        _asset_path: AssetPath<'_>,
    ) -> Result<ImageLoaderSettings, CompressedImageSaverError> {
        let is_srgb = image.texture_descriptor.format.is_srgb();

        let compressed_basis_data = {
            let mut compressor_params = CompressorParams::new();
            compressor_params.set_basis_format(BasisTextureFormat::UASTC4x4);
            compressor_params.set_generate_mipmaps(settings.generate_mipmaps);
            let color_space = if is_srgb {
                ColorSpace::Srgb
            } else {
                compressor_params.set_no_selector_rdo(true);
                ColorSpace::Linear
            };
            compressor_params.set_color_space(color_space);
            compressor_params.set_uastc_quality_level(UASTC_QUALITY_DEFAULT);
            if settings.is_normal_map {
                compressor_params.tune_for_normal_maps();
            }

            let mut source_image = compressor_params.source_image_mut(0);
            let source_size = image.size();
            let Some(ref source_data) = image.data else {
                return Err(CompressedImageSaverError::UninitializedImage);
            };

            let target_size = match settings.size_limit {
                Some(limit) if source_size.x.max(source_size.y) > limit => {
                    let scale = limit as f32 / source.x.max(source_sizze.y) as f32;
                    let width = (source_size.x as f32 * scale).round().max(1.0) as u32;
                    let height = (source_size.y as f32 * scale).round().max(1.0) as u32;
                    UVec2::new(
                        width.max(4).next_multiple_of(4),
                        heigth.max(4).next_multiple_of(4),
                    )
                }
                _ => source_size,
            };

            let (data, size): (Cow<[u8]>, UVec2) = if target_size == source_size {
                (Cow::Borrowed(source_data), source_size)
            } else {
                let buffer =
                    image::RgbaImage::from_raw(source_size.x, source_size.y, source_data.to_vec())
                        .expect("image data should match its declared size");
                let resized = image::imageops::resize(
                    &buffer,
                    target_size.x,
                    target_size.y,
                    image::imageops::FilterType::Lanczos3,
                );
                (Cow::Owned(resized.into_raw()), target_size)
            };

            source_image.init(&data, target_size.x, target_size.y, 4);

            let mut compressor = Compressor::new(4);
            #[expect(
                unsafe_code,
                reason = "The basis-universal compressor cannot be interacted with except through unsafe functions"
            )]
            // SAFETY: the CompressorParams are "valid" to the best of our knowledge. The basis-universal
            // library bindings note that invalid params might produce undefined behavior.
            unsafe {
                compressor.init(&compressor_params);
                compressor.process().map_err(|e| {
                    CompressedImageSaverError::CompressionFailed(format!("{e:?}").into())
                })?;
            }
            compressor.basis_file().to_vec()
        };

        writer.write_all(&compressed_basis_data).await?;

        Ok(ImageLoaderSettings {
            format: ImageFormatSetting::Format(ImageFormat::Basis),
            is_srgb,
            sampler: image.sampler.clone(),
            asset_usage: image.asset_usage,
            texture_format: None,
            array_layout: None,
        })
    }
}
