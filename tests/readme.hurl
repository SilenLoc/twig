# Test that README is displayed in repo view

# First get a ticket
POST {{target}}/api/auth/ticket
X-API-Key: {{api_key}}

HTTP 200
[Captures]
ticket: jsonpath "$.ticket"

# Signup a new user for this test
POST {{target}}/api/auth/signup
Content-Type: application/json
{
    "ticket": "{{ticket}}",
    "username": "readmeuser",
    "email": "readmeuser@example.com",
    "password": "readmepassword123"
}

HTTP 200

# Login to get auth token
POST {{target}}/api/auth/login
Authorization: Basic cmVhZG1ldXNlcjpyZWFkbWVwYXNzd29yZDEyMw==

HTTP 200
[Captures]
auth_token: jsonpath "$.token"

# Create namespace
POST {{target}}/api/auth/namespace
Authorization: Bearer {{auth_token}}
Content-Type: application/json
{
    "name": "test_readme"
}

HTTP 200

# Now create the repo with authentication
POST {{target}}/init
Authorization: Basic cmVhZG1ldXNlcjpyZWFkbWVwYXNzd29yZDEyMw==
Content-Type: application/x-www-form-urlencoded
[Form]
namespace: test_readme
repo: readme_repo
branch: main

HTTP 200

# Push a README.md to the repo with test description
# README content: "# Test Repository\n\nThis README was created by the hurl test suite to verify README display functionality."

# Check that the repo view is accessible and contains expected structure
GET {{target}}/test_readme/readme_repo

HTTP 200
[Asserts]
header "Content-Type" contains "text/html"
body contains "Commits"
