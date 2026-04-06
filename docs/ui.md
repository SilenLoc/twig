# UI Documentation

Fig provides a web interface for browsing namespaces, repositories, and managing your account.

## Authentication in the UI

The UI uses **session cookies** for authentication. After logging in, the server sets a `session` cookie that is used for all subsequent authenticated requests.

### Authentication by Action

| Action | Auth Required | How It Works |
|--------|--------------|--------------|
| View namespaces | No | Public read access |
| View repositories | No | Public read access |
| View commits/README | No | Public read access |
| Create namespace | Yes | logged in |
| Create repository | Yes | logged in |
| Generate signup ticket | Yes | API key (admin only) |
| Signup | Yes | Valid ticket (one-time use) |
| Login | Yes | Username/password form |
| Logout | Yes | Clears session |

### Authentication Flow

1. **Get Signup Ticket** (`/auth/ticket`) - Admin enters API key to generate a ticket
2. **Create Account** (`/auth/signup`) - User enters ticket, username, and password
3. **Log In** (`/auth/login`) - User enters credentials, session cookie is set
4. **Create Namespace** (`/auth/namespace`) - Cookie is sent automatically by browser
5. **Create Repository** (`/{namespace}`) - Cookie is sent with form submission

## Pages

### Home (`/`)

Displays a list of all namespaces with their owners.

**Features:**
- Search namespaces by name
- Create new namespace button (when logged in)
- Links to each namespace page

### Namespace Page (`/{namespace}`)

Displays all repositories within a namespace.

**Features:**
- Breadcrumb navigation
- Search repositories by name
- "Create repo" button (visible only if you have access)
- Repository list showing last commit time
- Links to each repository

### Create Repository Form (`/{namespace}/create-repo-form`)

HTMX-loaded form for creating a new repository.

**Fields:**
- Repository Name (required)
- Default Branch (defaults to "main")

### Repository Page (`/{namespace}/{repo}`)

Displays repository details.

**Features:**
- Breadcrumb navigation
- Rendered Markdown (if present)
- Commit history with:
  - Short commit hash
  - Author name
  - Commit date
  - Commit message

### Authentication Pages

#### Ticket Page (`/auth/ticket`)
- Form to generate a signup ticket
- Requires API key

#### Signup Page (`/auth/signup`)
- Form to create a new account
- Requires valid, unused ticket

#### Login Page (`/auth/login`)
- Form to log in
- Sets session cookie on success
- Redirects to namespace creation if user has no namespaces

#### Namespace Creation Page (`/auth/namespace`)
- Form to create a new namespace
- Requires session cookie

## Authentication State

The UI shows different content based on authentication state:

**Logged Out:**
- "Login" and "Signup" links in navigation
- Can browse namespaces and repositories
- Cannot create namespaces or repositories

**Logged In:**
- Username display and "Logout" link in navigation
- Can create namespaces
- Can create repositories in namespaces you own
