use git2::{Repository, Signature, IndexAddOption};
use anyhow::Result;

pub struct PhoenixGit {
    repo: Repository,
}

impl PhoenixGit {
    pub fn open() -> Result<Self> {
        let repo = Repository::open(".")?;
        Ok(Self { repo })
    }

    pub fn commit_fix(&self, file_path: &str, message: &str) -> Result<()> {
        let mut index = self.repo.index()?;
        index.add_all([file_path].iter(), IndexAddOption::DEFAULT, None)?;
        index.write()?;

        let tree_id = index.write_tree()?;
        let tree = self.repo.find_tree(tree_id)?;

        let sig = self.repo.signature()?; // Uses global git config (user.name/email)
        let parent_commit = self.repo.head()?.peel_to_commit()?;

        self.repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            message,
            &tree,
            &[&parent_commit],
        )?;

        Ok(())
    }

    pub fn push(&self) -> Result<()> {
        let mut remote = self.repo.find_remote("origin")?;
        // Note: Pushing usually requires credentials (SSH/HTTPS).
        // For local automation, we'll assume the environment is already authenticated
        // or using a credential helper.
        remote.push(&["refs/heads/main:refs/heads/main"], None)?;
        Ok(())
    }
}
