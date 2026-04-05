# Test signup with secure API key and specific credentials
# Uses API_KEY "secure" to signup user "default" with password "very-secure-pw"

# Step 1: Get ticket with secure API key
POST {{target}}/api/auth/ticket
X-API-Key: secure

HTTP 200
[Captures]
ticket: jsonpath "$.ticket"

# Step 2: Signup with valid ticket
POST {{target}}/api/auth/signup
Content-Type: application/json
{
    "ticket": "{{ticket}}",
    "username": "default",
    "email": "default@example.com",
    "password": "very-secure-pw"
}

HTTP 200
[Asserts]
jsonpath "$.username" == "default"
jsonpath "$.user_id" exists

# Step 3: Verify login works with the new credentials
POST {{target}}/api/auth/login
Authorization: Basic {{base64_default_password}}

HTTP 200
[Asserts]
jsonpath "$.token" exists
jsonpath "$.username" == "default"
