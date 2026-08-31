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

or 

```bash
git clone http://your-fig-server/namespace/repo
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

### 1. Create Account

1. Get a signup invite from your Fig administrator
2. Sign up at `http://your-fig-server/auth/signup`
3. Remember your username and password

### 2. Create Namespace

1. Log in at `http://your-fig-server/auth/login`
2. Create a namespace at `http://your-fig-server/auth/namespace`

### 3. Create Repository

Via web UI:
1. Go to `http://your-fig-server/namespace`
2. Click "Create Repository"

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