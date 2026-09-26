//! Mark decoder read failures separately from destination write failures.
//! Legacy encryption can only detect a bad password through a checksum failure.
use std::{
    fmt,
    io::{self, Read},
};

#[derive(Debug)]
pub(super) struct EncryptedReadError;

impl fmt::Display for EncryptedReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Incorrect password or damaged encrypted archive")
    }
}
impl std::error::Error for EncryptedReadError {}

pub(super) struct PasswordReader<R> {
    pub reader: R,
    pub encrypted: bool,
}

impl<R: Read> Read for PasswordReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.reader.read(buf).map_err(|error| {
            if self.encrypted
                && matches!(
                    error.kind(),
                    io::ErrorKind::InvalidData
                        | io::ErrorKind::Other
                        | io::ErrorKind::UnexpectedEof
                )
            {
                io::Error::other(EncryptedReadError)
            } else {
                error
            }
        })
    }
}

pub(super) fn is_password_read_error(error: &io::Error) -> bool {
    error
        .get_ref()
        .is_some_and(|error| error.is::<EncryptedReadError>())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct FailingReader(io::ErrorKind);
    impl Read for FailingReader {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::from(self.0))
        }
    }

    #[test]
    fn only_encrypted_decoder_failures_offer_a_password_retry() {
        for encrypted in [false, true] {
            for kind in [
                io::ErrorKind::InvalidData,
                io::ErrorKind::PermissionDenied,
                io::ErrorKind::Interrupted,
            ] {
                let error = PasswordReader {
                    reader: FailingReader(kind),
                    encrypted,
                }
                .read(&mut [0])
                .unwrap_err();
                assert_eq!(
                    is_password_read_error(&error),
                    encrypted && kind == io::ErrorKind::InvalidData
                );
            }
        }
    }
}
