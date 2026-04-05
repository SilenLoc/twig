# Test authentication endpoints
# New flow: Get Ticket (with API Key) → Signup (with ticket) → Login → Create Namespace (with auth)

# Test 1: Get ticket without API key should fail
POST {{target}}/api/auth/ticket

HTTP 401

# Test 2: Get ticket with invalid API key should fail
POST {{target}}/api/auth/ticket
X-API-Key: invalid_key

HTTP 401

# Test 3: Get ticket with valid API key should succeed
POST {{target}}/api/auth/ticket
X-API-Key: {{api_key}}

HTTP 200
[Captures]
ticket: jsonpath "$.ticket"

# Test 4: Signup without ticket should fail
POST {{target}}/api/auth/signup
Content-Type: application/json
{
    "username": "testuser",
    "email": "testuser@example.com",
    "password": "testpassword123"
}

HTTP 401

# Test 5: Signup with invalid ticket should fail
POST {{target}}/api/auth/signup
Content-Type: application/json
{
    "ticket": "invalid-ticket",
    "username": "testuser",
    "email": "testuser@example.com",
    "password": "testpassword123"
}

HTTP 401

# Test 6: Signup with valid ticket should succeed
POST {{target}}/api/auth/signup
Content-Type: application/json
{
    "ticket": "{{ticket}}",
    "username": "testuser",
    "email": "testuser@example.com",
    "password": "testpassword123"
}

HTTP 200
[Captures]
user_id: jsonpath "$.user_id"
username: jsonpath "$.username"

# Test 7: Signup with same ticket (already used) should fail
POST {{target}}/api/auth/signup
Content-Type: application/json
{
    "ticket": "{{ticket}}",
    "username": "testuser2",
    "email": "testuser2@example.com",
    "password": "testpassword123"
}

HTTP 401

# Test 8: Signup with same username should fail (get new ticket first)
POST {{target}}/api/auth/ticket
X-API-Key: {{api_key}}

HTTP 200
[Captures]
ticket2: jsonpath "$.ticket"

POST {{target}}/api/auth/signup
Content-Type: application/json
{
    "ticket": "{{ticket2}}",
    "username": "testuser",
    "email": "testuser2@example.com",
    "password": "testpassword123"
}

HTTP 409

# Test 9: Login with wrong password should fail
POST {{target}}/api/auth/login
Authorization: Basic {{base64_testuser_wrongpassword}}

HTTP 401

# Test 10: Login with correct credentials should succeed
POST {{target}}/api/auth/login
Authorization: Basic {{base64_testuser_password}}

HTTP 200
[Captures]
auth_token: jsonpath "$.token"

# Test 11: Create namespace without auth should fail
POST {{target}}/api/auth/namespace
Content-Type: application/json
{
    "name": "testnamespace"
}

HTTP 401

# Test 12: Create namespace with valid auth should succeed
POST {{target}}/api/auth/namespace
Authorization: Bearer {{auth_token}}
Content-Type: application/json
{
    "name": "testnamespace"
}

HTTP 200
[Captures]
namespace_id: jsonpath "$.namespace_id"

# Test 13: Create namespace with same name should fail
POST {{target}}/api/auth/namespace
Authorization: Bearer {{auth_token}}
Content-Type: application/json
{
    "name": "testnamespace"
}

HTTP 409

# Test 14: Init repo without auth should fail
POST {{target}}/init
Content-Type: application/x-www-form-urlencoded
[Form]
namespace: testnamespace
repo: testrepo

HTTP 401

# Test 15: Init repo with auth should succeed
POST {{target}}/init
Authorization: Basic {{base64_testuser_password}}
Content-Type: application/x-www-form-urlencoded
[Form]
namespace: testnamespace
repo: testrepo

HTTP 200

# Test 16: Logout should succeed
POST {{target}}/api/auth/logout
Authorization: Bearer {{auth_token}}

HTTP 200

# Test 17: Create namespace after logout should fail (token invalidated)
POST {{target}}/api/auth/namespace
Authorization: Bearer {{auth_token}}
Content-Type: application/json
{
    "name": "testnamespace2"
}

HTTP 401
