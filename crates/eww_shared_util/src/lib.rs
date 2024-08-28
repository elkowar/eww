pub mod locale;
pub mod span;
pub mod wrappers;

pub use locale::*;
pub use span::*;
pub use wrappers::*;

#[macro_export]
macro_rules! snapshot_debug {
    ( $($name:ident => $test:expr),* $(,)?) => {
        $(
            #[test]
            fn $name() { ::insta::assert_debug_snapshot!($test); }
        )*
    };
}
#[macro_export]
macro_rules! snapshot_string {
    ( $($name:ident => $test:expr),* $(,)?) => {
        $(
            #[test]
            fn $name() { ::insta::assert_snapshot!($test); }
        )*
    };
}

#[macro_export]
macro_rules! snapshot_ron {
    ( $($name:ident => $test:expr),* $(,)?) => {
        $(
            #[test]
            fn $name() {
                ::insta::with_settings!({sort_maps => true}, {
                    ::insta::assert_ron_snapshot!($test);
                });
            }
        )*
    };
}
