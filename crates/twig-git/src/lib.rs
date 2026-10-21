//! Git and bare-repository functionality shared by Twig's application layer.

pub mod backend;
pub mod bare;
pub mod hooks;
pub mod repo;
pub mod reserved;

pub use git2::{Error as GitError, Oid};
pub use repo::{
    RepositoryError, create_repo, create_repo_with_author, delete_repository, is_bare_repository,
    is_repository, move_repository,
};
