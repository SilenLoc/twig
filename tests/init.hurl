# First signup and create namespace
POST {{target}}/auth/signup
X-API-Key: {{api_key}}
Content-Type: application/json
{
    "username": "inituser",
    "password": "initpassword123"
}

HTTP 200
[Captures]
ticket: jsonpath "$.ticket"

POST {{target}}/auth/namespace
Content-Type: application/json
{
    "name": "init_namespace",
    "ticket": "{{ticket}}"
}

HTTP 200

# Now init repo with authentication
POST {{target}}/init
Authorization: Basic {{base64("inituser:initpassword123")}}
Content-Type: application/x-www-form-urlencoded
[Form]
namespace: init_namespace
repo: test_repo

HTTP 200