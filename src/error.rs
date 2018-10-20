//! Module holding our error type.

use std::io;
use SectionKind;

quick_error! {
    /// Represents the possible errors we could encounter.
    #[derive(Debug)]
    pub enum Error {
        /// An error was encountered performing IO operations.
        Io(err: io::Error) {
            from()
            display("IO error: {}", err)
        }

        /// We were asked to write more sections than can fit in a DOL file.
        TooManySections(kind: SectionKind) {
            display("Too many sections of kind {:?}", kind)
        }
    }
}
