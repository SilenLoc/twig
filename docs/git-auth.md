# Git Authentication Guide

This guide explains how to authenticate with Fig when using Git operations.

## Overview

Fig supports Git operations over HTTP with different authentication requirements:

- **Read operations** (clone, fetch): No authentication required
- **Write operations** (push): Requires authentication
- **Repository initialization**: Requires authentication

## Authentication Methods

### 1. Git Operations (Clone, Fetch, Push)

Git operations use **HTTP Basic Authentication** with your username and password.

#### Read Operations (No Auth Required)

```bash
# Clone a public repository
git clone http://your-fig-server/namespace/repo.git

# Fetch updates
git fetch origin
```

#### Write Operations (Auth Required)

```bash
# Push changes (will prompt for credentials)
git push origin main

# Or include credentials in the URL
git push http://username:password@your-fig-server/namespace/repo.git main
```

**Note:** Using credentials in the URL is convenient but not secure. Use credential helpers instead.

### 2. Credential Helpers (Recommended)

Store credentials securely so you don't need to enter them every time:

#### Git Credential Cache (Temporary)

```bash
# Cache credentials for 1 hour (3600 seconds)
git config --global credential.helper 'cache --timeout=3600'

# Now Git will remember your credentials temporarily
git push origin main
```

#### Git Credential Store (Permanent)

```bash
# Store credentials in ~/.git-credentials (plaintext!)
git config --global credential.helper store

# First push will save credentials
git push origin main
```

#### macOS Keychain

```bash
git config --global credential.helper osxkeychain
```

#### Windows Credential Manager

```bash
git config --global credential.helper manager
```

### 3. Per-Repository Configuration

Configure credentials for a specific repository:

```bash
cd your-repo
git config credential.helper 'cache --timeout=3600'
```

## Setting Up Git with Fig

### Step 1: Create an Account

1. Get a signup ticket from your Fig administrator (or via `/auth/ticket` if you have an API key)
2. Sign up at `http://your-fig-server/auth/signup`
3. Remember your username and password

### Step 2: Create a Namespace

1. Log in at `http://your-fig-server/auth/login`
2. Create a namespace at `http://your-fig-server/auth/namespace`

### Step 3: Initialize a Repository

Via web UI:
1. Go to `http://your-fig-server/namespace`
2. Click "Create Repository"

Via API (using Basic Auth):
```bash
curl -X POST http://your-fig-server/init \
  -u username:password \
  -d "namespace=my-namespace" \
  -d "repo=my-repo"
```

### Step 4: Clone and Push

```bash
# Clone the empty repository
git clone http://your-fig-server/my-namespace/my-repo.git
cd my-repo

# Add some files
echo "# My Project" > README.md
git add README.md
git commit -m "Initial commit"

# Push (you'll be prompted for credentials)
git push origin main
```

## Troubleshooting

### "Authentication failed" on push

1. Verify your username and password
2. Check that you have access to the namespace
3. Try clearing credential cache: `git credential-cache exit`

### "Access denied to namespace"

You don't have permission to push to this namespace. The namespace owner needs to grant you access.

### "Repository not found"

The repository doesn't exist. Initialize it first via the web UI or `/init` endpoint.

### Credentials not being saved

Check your credential helper:
```bash
git config --list | grep credential
```

## Security Best Practices

1. **Never commit credentials** to your repository
2. **Use credential helpers** instead of typing passwords repeatedly
3. **Use HTTPS** (or configure TLS/SSL on your Fig server)
4. **Clear credential cache** on shared computers: `git credential-cache exit`
5. **Use strong passwords** - Fig uses Argon2 for secure password hashing

## API vs Git Authentication

Different endpoints use different authentication methods:

| Endpoint Type | Example | Auth Method |
|--------------|---------|-------------|
| Git operations | `git push` | Basic Auth (username:password) |
| Web UI | `/auth/login` | Session cookie |
| API | `/api/auth/login` | Basic Auth → Bearer token |
| Init repo | `POST /init` | Basic Auth |

## Quick Reference

```bash
# Clone (no auth)
git clone http://server/namespace/repo.git

# Push with explicit credentials
git push http://username:password@server/namespace/repo.git main

# Configure credential helper
git config credential.helper 'cache --timeout=3600'

# Clear cached credentials
git credential-cache exit
```
