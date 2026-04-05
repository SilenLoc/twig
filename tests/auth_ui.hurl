# Test authentication UI endpoints
# New flow: Get Ticket (with API Key) → Signup (with ticket) → Login (sets cookie) → Create Namespace

# Test 1: Ticket page should load
GET {{target}}/auth/ticket

HTTP 200
[Asserts]
body contains "Get Signup Ticket"
body contains "API Key"

# Test 2: Signup page should load
GET {{target}}/auth/signup

HTTP 200
[Asserts]
body contains "Create Account"
body contains "Signup Ticket"

# Test 3: Login page should load
GET {{target}}/auth/login

HTTP 200
[Asserts]
body contains "Log In"
body contains "Username"

# Test 4: Namespace page should load
GET {{target}}/auth/namespace

HTTP 200
[Asserts]
body contains "Create Namespace"

# Test 5: Generate ticket with invalid API key should fail
POST {{target}}/auth/ticket
Content-Type: application/x-www-form-urlencoded
[Form]
api_key: invalid_key

HTTP 401
[Asserts]
body contains "Invalid API key"

# Test 6: Generate ticket with valid API key should succeed
POST {{target}}/auth/ticket
Content-Type: application/x-www-form-urlencoded
[Form]
api_key: {{api_key}}

HTTP 200
[Asserts]
body contains "Ticket Generated!"

# Capture the ticket from the HTML response (it's in a <code> block)
[Captures]
uiticket: regex "<code[^>]*>([a-f0-9-]{36,})</code>"

# Test 7: Signup with invalid ticket should fail
POST {{target}}/auth/signup
Content-Type: application/x-www-form-urlencoded
[Form]
ticket: invalid-ticket-code
username: uiuser
email: uiuser@example.com
password: password123

HTTP 401
[Asserts]
body contains "Invalid ticket"

# Test 8: Signup with valid ticket should succeed
POST {{target}}/auth/signup
Content-Type: application/x-www-form-urlencoded
[Form]
ticket: {{uiticket}}
username: uiuser
email: uiuser@example.com
password: password123

HTTP 200
[Asserts]
body contains "Account Created!"
body contains "Welcome, uiuser!"

# Test 9: Signup with same ticket (already used) should fail
POST {{target}}/auth/signup
Content-Type: application/x-www-form-urlencoded
[Form]
ticket: {{uiticket}}
username: uiuser2
email: uiuser2@example.com
password: password123

HTTP 401
[Asserts]
body contains "Ticket has already been used"

# Test 10: Signup form with duplicate username should fail
# First generate a new ticket
POST {{target}}/auth/ticket
Content-Type: application/x-www-form-urlencoded
[Form]
api_key: {{api_key}}

HTTP 200
[Captures]
uiticket2: regex "<code[^>]*>([a-f0-9-]{36,})</code>"

# Try to signup with duplicate username
POST {{target}}/auth/signup
Content-Type: application/x-www-form-urlencoded
[Form]
ticket: {{uiticket2}}
username: uiuser
email: uiuser2@example.com
password: password123

HTTP 409
[Asserts]
body contains "Username already exists"

# Test 11: Login form with wrong password should fail
POST {{target}}/auth/login
Content-Type: application/x-www-form-urlencoded
[Form]
username: uiuser
password: wrongpassword

HTTP 401
[Asserts]
body contains "Invalid credentials"

# Test 12: Create namespace without session cookie should fail
# This test MUST come before login to ensure no session cookie is stored
POST {{target}}/auth/namespace
Content-Type: application/x-www-form-urlencoded
[Form]
name: shouldfail_ns

HTTP 401
[Asserts]
body contains "Not logged in"

# Test 13: Login form with correct credentials should succeed and set session cookie
POST {{target}}/auth/login
Content-Type: application/x-www-form-urlencoded
[Form]
username: uiuser
password: password123

HTTP 200
[Asserts]
body contains "Login Successful!"
body contains "Welcome back, uiuser!"
body contains "You are now logged in"
[Captures]
session_cookie: header "Set-Cookie"

# Test 14: Create namespace with session cookie should succeed
POST {{target}}/auth/namespace
Content-Type: application/x-www-form-urlencoded
Cookie: {{session_cookie}}
[Form]
name: uinamespace

HTTP 200
[Asserts]
body contains "Namespace 'uinamespace' created successfully"

# Test 15: Create namespace with duplicate name should fail
POST {{target}}/auth/namespace
Content-Type: application/x-www-form-urlencoded
Cookie: {{session_cookie}}
[Form]
name: uinamespace

HTTP 409
[Asserts]
body contains "Namespace already exists"

# Test 16: Create another namespace (users can create multiple)
POST {{target}}/auth/namespace
Content-Type: application/x-www-form-urlencoded
Cookie: {{session_cookie}}
[Form]
name: uinamespace2

HTTP 200
[Asserts]
body contains "Namespace 'uinamespace2' created successfully"
