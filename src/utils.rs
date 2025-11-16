pub fn is_root() -> bool {
    #[cfg(unix)]
    {
        unsafe extern "C" {
            unsafe fn geteuid() -> u32;
        }

        unsafe { geteuid() == 0 }
    }

    #[cfg(not(unix))]
    {
        false
    }
}
