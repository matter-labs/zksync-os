pub trait Localizable {
    fn get_location(&self) -> ErrorLocation;
}

#[cfg(feature = "error_origins")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ErrorLocation {
    pub line: u32,
    pub file: &'static str,
}

#[cfg(not(feature = "error_origins"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ErrorLocation;

impl ErrorLocation {
    #[allow(unused_variables)]
    pub fn new(file: &'static str, line: u32) -> Self {
        #[cfg(feature = "error_origins")]
        {
            Self { file, line }
        }
        #[cfg(not(feature = "error_origins"))]
        {
            Self {}
        }
    }
}

#[macro_export]
macro_rules! location {
    () => {
        $crate::system::errors::location::ErrorLocation::new(file!(), line!())
    };
}

impl core::fmt::Display for ErrorLocation {
    #[cfg_attr(not(feature = "error_origins"), allow(unused_variables))]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        #[cfg(feature = "error_origins")]
        {
            let Self { line, file } = self;
            write!(f, "{file}:{line}")
        }
        #[cfg(not(feature = "error_origins"))]
        {
            Ok(())
        }
    }
}
