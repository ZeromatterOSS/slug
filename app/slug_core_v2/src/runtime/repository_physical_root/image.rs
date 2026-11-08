//! Fixed bridge into the trusted input-image read/copy kernels. No directory
//! File/raw descriptor or generic IO callback is returned to source consumers.
use super::*;
use crate::runtime::input_image::InputImageCaptureError;
use crate::runtime::input_image::ScopedImageSource;
impl RepositoryPhysicalRoot {
    pub(in crate::runtime) fn image_source(
        self: &Arc<Self>,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<ScopedImageSource, InputImageCaptureError> {
        self.check_origin(scope, active)
            .map_err(|_| InputImageCaptureError::Cancelled)?;
        // One retained source duplicate plus conservative bounded component-
        // walk/dirent/file-copy overlap, kept through all image/ELF work. The
        // image output's own descriptor/logical budgets remain independent.
        let credit = self
            .resources
            .reserve(
                add(std::mem::size_of::<ScopedImageSource>(), IO_SCRATCH)
                    .map_err(|_| InputImageCaptureError::LimitExceeded)?,
                8,
                0,
            )
            .map_err(|_| InputImageCaptureError::LimitExceeded)?;
        self.validate_backing()
            .map_err(|_| InputImageCaptureError::StaleSource)?;
        let file = self.file.try_clone()?;
        self.validate_image_directory(&file, scope, active)?;
        ScopedImageSource::from_physical(file, self.clone(), scope.clone(), credit)
    }
    pub(in crate::runtime) fn validate_image_directory(
        &self,
        file: &File,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<(), InputImageCaptureError> {
        self.check_origin(scope, active)
            .map_err(|_| InputImageCaptureError::Cancelled)?;
        self.validate_backing()
            .map_err(|_| InputImageCaptureError::StaleSource)?;
        if backing_identity(file, self.mode).map_err(|_| InputImageCaptureError::StaleSource)?
            != self.identity
        {
            return Err(InputImageCaptureError::StaleSource);
        }
        self.check_origin(scope, active)
            .map_err(|_| InputImageCaptureError::Cancelled)
    }
}
