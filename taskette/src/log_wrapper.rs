#[macro_export]
macro_rules! dispatch_log {
    ( $level:ident, $( $arg:expr ),+ ) => {
        {
            #[cfg(feature = "log")]
            log::$level!( $( $arg ),+ );
            #[cfg(feature = "defmt")]
            defmt::$level!( $( $arg ),+ );
        }
    };
}

#[macro_export]
macro_rules! info {
    ( $( $arg:expr ),+ ) => { crate::dispatch_log!(info, $( $arg ),+ ) };
}

#[macro_export]
macro_rules! debug {
    ( $( $arg:expr ),+ ) => { crate::dispatch_log!(debug, $( $arg ),+ ) };
}

#[macro_export]
macro_rules! trace {
    ( $( $arg:expr ),+ ) => { crate::dispatch_log!(trace, $( $arg ),+ ) };
}
