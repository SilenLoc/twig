# Git Backend Documentation

Fig provides a Git HTTP backend that allows you to host Git repositories and interact with them using standard Git commands.

## Authentication

Git operations use **HTTP Basic Authentication** with your username and password.

| Operation | Authentication |
|-----------|---------------|
| Clone | None (public read) |
| Fetch | None (public read) |
| Push | Basic Auth required |

## Clone a Repository

```bash
git clone http://your-fig-server/namespace/repo.git
```

## Fetch Updates

```bash
git fetch origin
```

## Push Changes

```bash
# Push with credential prompt
git push origin main

# Or include credentials in the URL
git push http://username:password@your-fig-server/namespace/repo.git main
```

**Note:** Using credentials in the URL is convenient but not secure. Use credential helpers instead.

## Credential Helpers

Store credentials securely so you don't need to enter them every time:

### Cache (Temporary)

```bash
# Cache credentials for 1 hour (3600 seconds)
git config --global credential.helper 'cache --timeout=3600'
```

### Store (Permanent - Plaintext)

```bash
# Store credentials in ~/.git-credentials
git config --global credential.helper store
```

### macOS Keychain

```bash
git config --global credential.helper osxkeychain
```

### Windows Credential Manager

```bash
git config --global credential.helper manager
```

## Complete Workflow

### 1. Create Account

1. Get a signup ticket from your Fig administrator
2. Sign up at `http://your-fig-server/auth/signup`
3. Remember your username and password

### 2. Create Namespace

1. Log in at `http://your-fig-server/auth/login`
2. Create a namespace at `http://your-fig-server/auth/namespace`

### 3. Create Repository

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

### 4. Clone and Push

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

## Access Control

- Users can only push to namespaces they own
- Read operations (clone/fetch) are public
- Each namespace is owned by the user who created it

## Troubleshooting

### "Authentication failed" on push

1. Verify your username and password
2. Check that you have access to the namespace
3. Try clearing credential cache: `git credential-cache exit`

### "Access denied to namespace"

You don't have permission to push to this namespace. You must be the namespace owner.

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
