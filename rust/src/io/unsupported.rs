// Platforms with no asynchronous I/O interface. Every queue fails to open here.
use super::{AsyncIo, Backend, Failure, Request, StreamCtx};
use std::{
    fs::{File, OpenOptions},
    io,
};

pub struct Native;

impl Native {
    pub fn new(
        _file: &File,
        _backend: Backend,
        _depth: usize,
        _slots: Vec<File>,
    ) -> io::Result<Self> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "no asynchronous I/O backend on this platform: pagecache and direct need POSIX AIO (macOS, FreeBSD), libaio or io_uring (Linux), or overlapped I/O (Windows)",
        ))
    }
}

impl Native {
    pub fn default_backend() -> Backend {
        Backend::Pagecache
    }

    pub fn configure_open(
        _o: &mut OpenOptions,
        _path: &std::path::Path,
        backend: Backend,
    ) -> io::Result<()> {
        if backend != Backend::Pagecache {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "this platform currently supports only the pagecache backend",
            ));
        }
        Ok(())
    }

    pub fn after_open(_file: &File, _path: &std::path::Path, _backend: Backend) -> io::Result<()> {
        Ok(())
    }

    pub fn open_slots(
        _path: &std::path::Path,
        _write: bool,
        _backend: Backend,
        _depth: usize,
    ) -> io::Result<Vec<File>> {
        Ok(Vec::new())
    }

    pub fn check_target(_path: &std::path::Path, _write: bool) -> crate::Result<()> {
        Ok(())
    }
}

impl AsyncIo for Native {
    fn stream<N, P, C>(
        &mut self,
        _ctx: StreamCtx<'_>,
        _next: N,
        _prepare: P,
        _complete: C,
    ) -> std::result::Result<(), Failure>
    where
        N: FnMut() -> Option<Request>,
        P: FnMut(Request, &mut [u8]),
        C: FnMut(Request),
    {
        unreachable!("Native::new never succeeds on this platform")
    }

    fn execute<C>(&mut self, _ctx: StreamCtx<'_>, _complete: C) -> std::result::Result<(), Failure>
    where
        C: FnMut(usize, Request),
    {
        unreachable!("Native::new never succeeds on this platform")
    }
}
