use std::fmt;

use flexi_logger::writers::LogWriter;

use crate::dep_mut;

pub struct StringLogWriter;

pub struct StringLog {
    logs: Vec<String>,
}

impl fmt::Debug for StringLog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StringLog").finish_non_exhaustive()
    }
}

impl StringLog {
    pub fn new() -> Self {
        Self { logs: Vec::new() }
    }

    pub fn _for_each<F>(&self, func: F)
    where
        F: FnMut(&String),
    {
        self.logs.iter().for_each(func);
    }

    pub fn push(&mut self, line: String) {
        self.logs.push(line);
    }
}

impl LogWriter for StringLogWriter {
    fn write(
        &self,
        now: &mut flexi_logger::DeferredNow,
        record: &log::Record,
    ) -> std::io::Result<()> {
        let line = format!(
            "[{}] {} - {}",
            record.level().as_str().to_uppercase(),
            now.now().format("%Y-%m-%d %H:%M:%S"),
            record.args()
        );
        match record.level() {
            log::Level::Error => {
                eprintln!("{}", line);
            }
            _ => {
                println!("{}", line);
            }
        }
        dep_mut!(StringLog, |log| log.push(line));
        Ok(())
    }

    fn flush(&self) -> std::io::Result<()> {
        // Write to file?
        Ok(())
    }
}
