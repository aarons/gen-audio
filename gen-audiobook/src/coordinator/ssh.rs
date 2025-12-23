//! SSH transport layer for worker communication.
//!
//! Uses system SSH and SFTP commands for maximum compatibility.

use super::config::WorkerConfig;
use anyhow::{Context, Result};
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;

/// SSH connection to a remote worker.
#[derive(Debug)]
pub struct SshConnection {
    /// Worker configuration.
    config: WorkerConfig,
    /// SSH timeout.
    timeout: Duration,
    /// Control socket path for connection multiplexing.
    control_socket: Option<std::path::PathBuf>,
}

impl SshConnection {
    /// Create a new SSH connection.
    pub fn new(config: WorkerConfig, timeout_secs: u64) -> Self {
        Self {
            config,
            timeout: Duration::from_secs(timeout_secs),
            control_socket: None,
        }
    }

    /// Get the worker name.
    pub fn name(&self) -> &str {
        &self.config.name
    }

    /// Build common SSH arguments.
    fn ssh_args(&self) -> Vec<String> {
        let mut args = vec![
            "-o".to_string(),
            "BatchMode=yes".to_string(),
            "-o".to_string(),
            "StrictHostKeyChecking=accept-new".to_string(),
            "-o".to_string(),
            format!("ConnectTimeout={}", self.timeout.as_secs()),
        ];

        // Use control master if available
        if let Some(ref socket) = self.control_socket {
            args.extend([
                "-o".to_string(),
                format!("ControlPath={}", socket.display()),
                "-o".to_string(),
                "ControlMaster=auto".to_string(),
                "-o".to_string(),
                "ControlPersist=60".to_string(),
            ]);
        }

        // Add SSH key if specified
        if let Some(key_path) = self.config.expanded_ssh_key() {
            args.extend(["-i".to_string(), key_path.to_string_lossy().to_string()]);
        }

        // Add port if non-default
        if self.config.port != 22 {
            args.extend(["-p".to_string(), self.config.port.to_string()]);
        }

        args
    }

    /// Establish a control master connection for multiplexing.
    pub async fn establish_control_master(&mut self) -> Result<()> {
        // Create a unique control socket path
        let socket_dir = std::env::temp_dir().join("gena-ssh");
        std::fs::create_dir_all(&socket_dir)
            .context("Failed to create control socket directory")?;

        let socket_path = socket_dir.join(format!(
            "{}_{}_{}",
            self.config.user, self.config.host, self.config.port
        ));

        self.control_socket = Some(socket_path.clone());

        // Start control master
        let mut args = vec![
            "-o".to_string(),
            "BatchMode=yes".to_string(),
            "-o".to_string(),
            format!("ControlPath={}", socket_path.display()),
            "-o".to_string(),
            "ControlMaster=yes".to_string(),
            "-o".to_string(),
            "ControlPersist=300".to_string(),
            "-N".to_string(),
            "-f".to_string(),
        ];

        if let Some(key_path) = self.config.expanded_ssh_key() {
            args.extend(["-i".to_string(), key_path.to_string_lossy().to_string()]);
        }

        if self.config.port != 22 {
            args.extend(["-p".to_string(), self.config.port.to_string()]);
        }

        args.push(self.config.ssh_target());

        let output = Command::new("ssh")
            .args(&args)
            .output()
            .await
            .context("Failed to start SSH control master")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("SSH control master failed: {}", stderr);
        }

        Ok(())
    }

    /// Close the control master connection.
    pub async fn close_control_master(&self) -> Result<()> {
        if let Some(ref socket) = self.control_socket {
            let args = vec![
                "-o".to_string(),
                format!("ControlPath={}", socket.display()),
                "-O".to_string(),
                "exit".to_string(),
                self.config.ssh_target(),
            ];

            let _ = Command::new("ssh")
                .args(&args)
                .output()
                .await;
        }
        Ok(())
    }

    /// Execute a command on the remote host.
    pub async fn exec(&self, command: &str) -> Result<String> {
        let mut args = self.ssh_args();
        args.push(self.config.ssh_target());
        args.push(command.to_string());

        let output = tokio::time::timeout(self.timeout, async {
            Command::new("ssh")
                .args(&args)
                .output()
                .await
        })
        .await
        .context("SSH command timed out")?
        .context("Failed to execute SSH command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("SSH command failed: {}", stderr);
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Execute a command with stdin input and return stdout.
    pub async fn exec_with_input(&self, command: &str, input: &[u8]) -> Result<Vec<u8>> {
        let mut args = self.ssh_args();
        args.push(self.config.ssh_target());
        args.push(command.to_string());

        let mut child = Command::new("ssh")
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("Failed to spawn SSH command")?;

        // Write input to stdin
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(input).await
                .context("Failed to write to SSH stdin")?;
        }

        // Wait for completion with timeout
        let output = tokio::time::timeout(self.timeout, child.wait_with_output())
            .await
            .context("SSH command timed out")?
            .context("Failed to get SSH output")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("SSH command failed: {}", stderr);
        }

        Ok(output.stdout)
    }

    /// Upload a file via SFTP.
    pub async fn upload(&self, local: &Path, remote: &str) -> Result<()> {
        let mut sftp_args = vec![
            "-b".to_string(),
            "-".to_string(), // Read commands from stdin
            "-o".to_string(),
            "BatchMode=yes".to_string(),
        ];

        // Use control master if available
        if let Some(ref socket) = self.control_socket {
            sftp_args.extend([
                "-o".to_string(),
                format!("ControlPath={}", socket.display()),
            ]);
        }

        if let Some(key_path) = self.config.expanded_ssh_key() {
            sftp_args.extend(["-i".to_string(), key_path.to_string_lossy().to_string()]);
        }

        if self.config.port != 22 {
            sftp_args.extend(["-P".to_string(), self.config.port.to_string()]);
        }

        sftp_args.push(self.config.ssh_target());

        // SFTP batch commands
        let batch_commands = format!(
            "put {} {}\nquit\n",
            local.display(),
            remote
        );

        let mut child = Command::new("sftp")
            .args(&sftp_args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("Failed to spawn SFTP")?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(batch_commands.as_bytes()).await
                .context("Failed to write SFTP commands")?;
        }

        let output = tokio::time::timeout(
            Duration::from_secs(300), // 5 min for uploads
            child.wait_with_output()
        )
        .await
        .context("SFTP upload timed out")?
        .context("SFTP failed")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("SFTP upload failed: {}", stderr);
        }

        Ok(())
    }

    /// Download a file via SFTP.
    pub async fn download(&self, remote: &str, local: &Path) -> Result<()> {
        let mut sftp_args = vec![
            "-b".to_string(),
            "-".to_string(),
            "-o".to_string(),
            "BatchMode=yes".to_string(),
        ];

        if let Some(ref socket) = self.control_socket {
            sftp_args.extend([
                "-o".to_string(),
                format!("ControlPath={}", socket.display()),
            ]);
        }

        if let Some(key_path) = self.config.expanded_ssh_key() {
            sftp_args.extend(["-i".to_string(), key_path.to_string_lossy().to_string()]);
        }

        if self.config.port != 22 {
            sftp_args.extend(["-P".to_string(), self.config.port.to_string()]);
        }

        sftp_args.push(self.config.ssh_target());

        let batch_commands = format!(
            "get {} {}\nquit\n",
            remote,
            local.display()
        );

        let mut child = Command::new("sftp")
            .args(&sftp_args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("Failed to spawn SFTP")?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(batch_commands.as_bytes()).await
                .context("Failed to write SFTP commands")?;
        }

        let output = tokio::time::timeout(
            Duration::from_secs(300),
            child.wait_with_output()
        )
        .await
        .context("SFTP download timed out")?
        .context("SFTP failed")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("SFTP download failed: {}", stderr);
        }

        Ok(())
    }

    /// Check if a remote file exists.
    pub async fn file_exists(&self, remote: &str) -> Result<bool> {
        let result = self.exec(&format!("test -f {} && echo yes || echo no", remote)).await?;
        Ok(result.trim() == "yes")
    }

    /// Create a remote directory.
    pub async fn mkdir(&self, remote: &str) -> Result<()> {
        self.exec(&format!("mkdir -p {}", remote)).await?;
        Ok(())
    }

    /// Remove a remote file.
    pub async fn remove(&self, remote: &str) -> Result<()> {
        self.exec(&format!("rm -f {}", remote)).await?;
        Ok(())
    }

    /// Test connection to the worker.
    pub async fn test_connection(&self) -> Result<()> {
        self.exec("echo ok").await?;
        Ok(())
    }
}

impl Drop for SshConnection {
    fn drop(&mut self) {
        // Try to clean up control socket
        if let Some(ref socket) = self.control_socket {
            let _ = std::fs::remove_file(socket);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ssh_args() {
        let config = WorkerConfig::new("test", "example.com", "user")
            .with_port(2222)
            .with_ssh_key("~/.ssh/test_key");

        let conn = SshConnection::new(config, 30);
        let args = conn.ssh_args();

        assert!(args.contains(&"-p".to_string()));
        assert!(args.contains(&"2222".to_string()));
        assert!(args.contains(&"-i".to_string()));
    }

    #[test]
    fn test_ssh_target() {
        let config = WorkerConfig::new("test", "192.168.1.1", "ubuntu");
        assert_eq!(config.ssh_target(), "ubuntu@192.168.1.1");
    }
}
