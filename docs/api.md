# API Documentation

Fig provides a RESTful API for programmatic access to all features.

## Authentication Methods by Action

Fig uses different authentication methods depending on the action you want to perform:

### API Key (X-API-Key Header)
**Purpose**: Generate signup tickets for new user registration  
**Used By**: `POST /api/auth/ticket`  
**Who Has It**: Server administrator (set via `API_KEY` environment variable)

The API Key is the most privileged credential. It is used to generate one-time signup tickets that allow new users to register.

### Ticket (Request Body)
**Purpose**: Create a new user account  
**Used By**: `POST /api/auth/signup`  
**Who Has It**: Anyone with a valid ticket (generated via API Key)

Tickets are single-use and are consumed when a user successfully signs up.

### Basic Auth (Username:Password)
**Purpose**: Initial login and repository initialization  
**Used By**:
- `POST /api/auth/login` - Exchange credentials for a Bearer token
- `POST /init` - Create a new repository
- Git push operations (via Git client)

Basic Auth uses your username and password created during signup.

### Bearer Token (Authorization Header)
**Purpose**: API actions after login  
**Used By**:
- `POST /api/auth/namespace` - Create a namespace
- `POST /api/auth/logout` - Log out and invalidate token

Bearer tokens are obtained by logging in via Basic Auth. They expire after 30 days of inactivity.

### Session Cookie
**Purpose**: Web UI authentication  
**Used By**:
- All web UI pages (`/auth/*`, `/`, `/{namespace}`, `/{namespace}/{repo}`)
- `POST /api/auth/logout` (alternative to Bearer token)

Session cookies are set when you log in via the web UI forms and are used for browser-based interactions.

### No Authentication
**Purpose**: Health checks and public read access  
**Used By**:
- `GET /health` - Health check
- `GET /up` - Health check
- Git clone and fetch operations

## Quick Reference

| Action | Auth Method | Endpoint |
|--------|-------------|----------|
| Generate signup ticket | API Key | `POST /api/auth/ticket` |
| Create account | Ticket | `POST /api/auth/signup` |
| Log in (get token) | Basic Auth | `POST /api/auth/login` |
| Create namespace | Bearer Token | `POST /api/auth/namespace` |
| Create repository | Basic Auth | `POST /init` |
| Log out | Bearer Token or Cookie | `POST /api/auth/logout` |
| Health check | None | `GET /health` |
| Git clone/fetch | None | `/{namespace}/{repo}.git` |
| Git push | Basic Auth | `/{namespace}/{repo}.git` |

## Endpoints

### Health Check

#### GET /health
Returns 200 OK when server is healthy.

**Response:**
```
HTTP 200 OK
```

#### GET /up
Alternative health check endpoint.

**Response:**
```
HTTP 200 OK
```

### Authentication

#### POST /api/auth/ticket
Generate a one-time signup ticket. Requires API Key.

**Headers:**
```
X-API-Key: your-api-key
```

**Response (200 OK):**
```json
{
  "ticket": "64-character-hex-string",
  "user_id": "",
  "username": ""
}
```

**Errors:**
- `401 Unauthorized` - Missing or invalid API key

#### POST /api/auth/signup
Create a new user account. Requires a valid ticket.

**Request Body:**
```json
{
  "ticket": "64-character-hex-string",
  "username": "myuser",
  "password": "mypassword123"
}
```

**Response (200 OK):**
```json
{
  "user_id": "uuid-string",
  "username": "myuser",
  "ticket": ""
}
```

**Errors:**
- `401 Unauthorized` - Missing, invalid, or already used ticket
- `400 Bad Request` - Username too short (< 3 chars) or password too short (< 8 chars)
- `409 Conflict` - Username already exists

#### POST /api/auth/login
Log in and receive a Bearer token.

**Headers:**
```
Authorization: Basic base64(username:password)
```

**Response (200 OK):**
```json
{
  "token": "64-character-hex-string",
  "user_id": "uuid-string",
  "username": "myuser"
}
```

**Errors:**
- `401 Unauthorized` - Missing credentials or invalid username/password

#### POST /api/auth/logout
Invalidate the current session token.

**Headers (one of):**
```
Authorization: Bearer your-token
```
or (for UI sessions):
```
Cookie: session=your-session-token
```

**Response (200 OK):**
```
Logged out
```

**Errors:**
- `401 Unauthorized` - Missing token
- `500 Internal Server Error` - Failed to invalidate token

### Namespaces

#### POST /api/auth/namespace
Create a new namespace. Requires Bearer token.

**Headers:**
```
Authorization: Bearer your-token
Content-Type: application/json
```

**Request Body:**
```json
{
  "name": "my-namespace"
}
```

**Response (200 OK):**
```json
{
  "namespace_id": "uuid-string",
  "name": "my-namespace"
}
```

**Errors:**
- `401 Unauthorized` - Missing or invalid token
- `400 Bad Request` - Namespace name too short (< 2 chars)
- `409 Conflict` - Namespace already exists
- `500 Internal Server Error` - Database error

### Repositories

#### POST /init
Initialize a new bare repository. Requires Basic Auth.

**Headers:**
```
Authorization: Basic base64(username:password)
Content-Type: application/x-www-form-urlencoded
```

**Request Body:**
```
namespace=my-namespace&repo=my-repo&branch=main
```

The `branch` parameter is optional and defaults to "main".

**Response (200 OK):**
```
Repository created
```

**Errors:**
- `401 Unauthorized` - Missing or invalid credentials
- `403 Forbidden` - User does not have access to namespace
- `500 Internal Server Error` - Failed to create repository

## Authentication Flow (API)

Complete workflow for creating a user and repository via API:

```bash
# 1. Get a signup ticket (requires API Key)
curl -X POST http://localhost:8080/api/auth/ticket \
  -H "X-API-Key: your-api-key"
# Response: {"ticket": "abc123..."}

# 2. Sign up with the ticket
curl -X POST http://localhost:8080/api/auth/signup \
  -H "Content-Type: application/json" \
  -d '{
    "ticket": "abc123...",
    "username": "myuser",
    "password": "mypassword123"
  }'
# Response: {"user_id": "...", "username": "myuser", "ticket": ""}

# 3. Log in to get Bearer token
curl -X POST http://localhost:8080/api/auth/login \
  -H "Authorization: Basic $(echo -n 'myuser:mypassword123' | base64)"
# Response: {"token": "xyz789...", "user_id": "...", "username": "myuser"}

# 4. Create a namespace
curl -X POST http://localhost:8080/api/auth/namespace \
  -H "Authorization: Bearer xyz789..." \
  -H "Content-Type: application/json" \
  -d '{"name": "my-namespace"}'
# Response: {"namespace_id": "...", "name": "my-namespace"}

# 5. Initialize a repository
curl -X POST http://localhost:8080/init \
  -H "Authorization: Basic $(echo -n 'myuser:mypassword123' | base64)" \
  -H "Content-Type: application/x-www-form-urlencoded" \
  -d "namespace=my-namespace&repo=my-repo&branch=main"
# Response: Repository created
```

## Error Responses

All errors return appropriate HTTP status codes:

- `400 Bad Request` - Invalid input data
- `401 Unauthorized` - Authentication required or failed
- `403 Forbidden` - Access denied to resource
- `409 Conflict` - Resource already exists
- `500 Internal Server Error` - Server error

## Token Security

- Tokens are 64-character hexadecimal strings
- Tokens expire after 30 days of inactivity
- Tokens are stored in a SQLite database
- Use `/api/auth/logout` to invalidate tokens before expiration
