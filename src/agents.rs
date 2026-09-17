use anyhow::{Context, Error, Result, anyhow};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const AGENTS_FILE: &str = "AGENTS.md";

#[derive(Default)]
pub(crate) struct InstructionContext {
    sources: Vec<Source>,
}

struct Source {
    path: PathBuf,
    content: String,
}

pub(crate) fn discover() -> Result<InstructionContext> {
    let start = std::env::current_dir().context("failed to determine launch directory")?;
    load_from(&start)
}

pub(crate) fn load_from(start: &Path) -> Result<InstructionContext> {
    load_from_with(start, |path| fs::read(path))
}

fn load_from_with<F>(start: &Path, mut read: F) -> Result<InstructionContext>
where
    F: FnMut(&Path) -> io::Result<Vec<u8>>,
{
    let mut directories: Vec<&Path> = start.ancestors().collect();
    directories.reverse();

    let mut sources = Vec::new();
    for directory in directories {
        let path = directory.join(AGENTS_FILE);
        let bytes = match read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(Error::new(error)
                    .context(format!("failed to read AGENTS.md at {}", path.display())));
            }
        };
        let content = String::from_utf8(bytes)
            .map_err(|_| anyhow!("AGENTS.md at {} is not valid UTF-8", path.display()))?;
        sources.push(Source { path, content });
    }

    Ok(InstructionContext { sources })
}

impl InstructionContext {
    pub(crate) fn for_request(&self, user: &str) -> String {
        if self.sources.is_empty() {
            return user.to_owned();
        }

        let mut request = String::from(
            "AGENTS.md instructions for this request:\n\
The sources below are ordered from broadest/outermost to nearest/most specific. \
A later, nearer AGENTS.md file overrides conflicting instructions from an earlier \
parent file. The explicit user request below takes precedence over every AGENTS.md \
instruction.\n\n",
        );
        for source in &self.sources {
            request.push_str("AGENTS.md source: ");
            request.push_str(&source.path.display().to_string());
            request.push('\n');
            request.push_str(&source.content);
            request.push_str("\n\n");
        }
        request.push_str("Explicit user request:\n");
        request.push_str(user);
        request
    }

    #[cfg(test)]
    pub(crate) fn from_contents(contents: &[&str]) -> Self {
        let sources = contents
            .iter()
            .enumerate()
            .map(|(index, content)| Source {
                path: PathBuf::from(format!("test/{index}/AGENTS.md")),
                content: (*content).to_owned(),
            })
            .collect();
        Self { sources }
    }
}

#[cfg(test)]
mod tests;
