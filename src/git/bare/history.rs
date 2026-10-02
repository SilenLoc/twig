//! Commit history types shared by the repository views.

use chrono::Utc;

/// A single commit in the repository history.
pub struct Commit {
    hash: String,
    author: String,
    date: chrono::DateTime<Utc>,
    message: String,
}

impl Commit {
    pub fn new(hash: String, author: String, date: chrono::DateTime<Utc>, message: String) -> Self {
        Self {
            hash,
            author,
            date,
            message,
        }
    }

    pub fn hash(&self) -> &str {
        &self.hash
    }

    pub fn author(&self) -> &str {
        &self.author
    }

    pub fn date(&self) -> &chrono::DateTime<Utc> {
        &self.date
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

/// How many commits a history listing returns.
pub struct Depth {
    pub depth: usize,
}

impl Depth {
    pub fn new(depth: usize) -> Self {
        Self { depth }
    }
}

impl Default for Depth {
    fn default() -> Self {
        Self::new(1000)
    }
}

/// Converts a libgit2 timestamp into a UTC datetime, falling back to the Unix
/// epoch for out-of-range values rather than failing the whole listing.
pub(super) fn chrono(git_time: git2::Time) -> chrono::DateTime<Utc> {
    chrono::DateTime::from_timestamp(git_time.seconds(), 0).unwrap_or(chrono::DateTime::UNIX_EPOCH)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chrono_handles_valid_timestamp() {
        let time = git2::Time::new(1_700_000_000, 0);
        let date = chrono(time);
        assert_eq!(date.timestamp(), 1_700_000_000);
    }

    #[test]
    fn test_chrono_handles_current_time() {
        let now = Utc::now().timestamp();
        let date = chrono(git2::Time::new(now, 0));
        assert_eq!(date.timestamp(), now);
    }
}
