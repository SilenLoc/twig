# First get a ticket
POST {{target}}/api/auth/ticket
X-API-Key: {{api_key}}

HTTP 200
[Captures]
ticket: jsonpath "$.ticket"

# Signup using the API endpoint with JSON
POST {{target}}/api/auth/signup
Content-Type: application/json
{
    "ticket": "{{ticket}}",
    "username": "inituser",
    "password": "initpassword123"
}

HTTP 200
[Captures]
user_id: jsonpath "$.user_id"

# Login to get auth token
POST {{target}}/api/auth/login
Authorization: Basic {{base64_inituser_password}}

HTTP 200
[Captures]
auth_token: jsonpath "$.token"

# Create namespace using API with Bearer token
POST {{target}}/api/auth/namespace
Authorization: Bearer {{auth_token}}
Content-Type: application/json
{
    "name": "init_namespace"
}

HTTP 200

# Now init repo with authentication
POST {{target}}/init
Authorization: Basic {{base64_inituser_password}}
Content-Type: application/x-www-form-urlencoded
[Form]
namespace: init_namespace
repo: test_repo

HTTP 200