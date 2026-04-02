use std::path::Path;

use chrono::Utc;

pub struct Commit {
    hash: String,
    author: String,
    date: chrono::DateTime<Utc>,
    commit_message: String,
}

impl Commit {
    pub fn new(
        hash: String,
        author: String,
        date: chrono::DateTime<Utc>,
        commit_message: String,
    ) -> Self {
        Self {
            hash,
            author,
            date,
            commit_message,
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

    pub fn commit_message(&self) -> &str {
        &self.commit_message
    }
}

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

pub fn get_commits(
    root: &str,
    namespace: &str,
    repo: &str,
    depth: Depth,
) -> Result<Vec<Commit>, git2::Error> {
    git_commits(root, namespace, repo, depth)
}

fn git_commits(
    root: &str,
    namespace: &str,
    repo: &str,
    depth: Depth,
) -> Result<Vec<Commit>, git2::Error> {
    let path = Path::new(root).join(namespace).join(repo);

    let repo = git2::Repository::open(&path).unwrap();

    // Get HEAD commit
    let obj = repo.head()?.resolve()?.peel(git2::ObjectType::Commit)?;
    let commit = obj
        .into_commit()
        .map_err(|_| git2::Error::from_str("Couldn't find commit"))?;

    // Create a revision walker
    let mut revwalk = repo.revwalk()?;
    revwalk.push(commit.id())?;

    let mut commits = Vec::new();

    revwalk.take(depth.depth).for_each(|oid_result| {
        let Ok(oid) = oid_result else {
            return;
        };

        let Ok(commit) = repo.find_commit(oid) else {
            return;
        };

        commits.push(Commit::new(
            oid.to_string(),
            commit.author().name().unwrap_or("").to_string(),
            chrono(commit.author().when()),
            commit.message().unwrap_or("").to_string(),
        ));
    });

    Ok(commits)
}

fn chrono(git_time: git2::Time) -> chrono::DateTime<Utc> {
    chrono::DateTime::from_timestamp(git_time.seconds(), 0).unwrap()
}

pub fn get_namespaces(root: &str) -> Result<Vec<String>, git2::Error> {
    let path = Path::new(root);
    let entries = std::fs::read_dir(path).unwrap();
    let mut namespaces = Vec::new();

    for entry in entries {
        let Ok(entry) = entry else {
            continue;
        };

        let Ok(namespace) = entry.file_name().into_string() else {
            continue;
        };
        namespaces.push(namespace);
    }

    Ok(namespaces)
}

pub fn get_repos(root: &str, namespace: &str) -> Result<Vec<String>, git2::Error> {
    let path = Path::new(root).join(namespace);
    let entries = std::fs::read_dir(path).unwrap();
    let mut repos = Vec::new();

    for entry in entries {
        let Ok(entry) = entry else {
            continue;
        };

        let Ok(repo) = entry.file_name().into_string() else {
            continue;
        };

        // check if they are repos
        let repo_path = Path::new(root).join(namespace).join(&repo);
        let is_repo = git2::Repository::open(&repo_path).is_ok();

        if !is_repo {
            continue;
        }

        repos.push(repo);
    }

    Ok(repos)
}
