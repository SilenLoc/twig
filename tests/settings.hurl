# Test Settings page functionality
# Requires: User signup and login flow

# Test 1: Settings page without login should show error
GET {{target}}/settings

HTTP 200
[Asserts]
body contains "Not logged in"

# Test 2: Generate ticket for settings test user
POST {{target}}/auth/ticket
Content-Type: application/x-www-form-urlencoded
[Form]
api_key: {{api_key}}

HTTP 200
[Captures]
settings_ticket: regex "<code[^>]*>([a-f0-9-]{36,})</code>"

# Test 3: Signup with valid ticket for settings user
POST {{target}}/auth/signup
Content-Type: application/x-www-form-urlencoded
[Form]
ticket: {{settings_ticket}}
username: settingsuser
email: settingsuser@example.com
password: settingspass123

HTTP 200
[Asserts]
body contains "Account Created!"

# Test 4: Login to get session cookie
POST {{target}}/auth/login
Content-Type: application/x-www-form-urlencoded
[Form]
username: settingsuser
password: settingspass123

HTTP 200
[Asserts]
body contains "Login Successful!"
[Captures]
settings_session: header "Set-Cookie"

# Test 5: Settings page with login should load successfully
GET {{target}}/settings
Cookie: {{settings_session}}

HTTP 200
[Asserts]
body contains "User Settings"
body contains "Profile Information"
body contains "Username"
body contains "settingsuser"
body contains "Email"
body contains "settingsuser@example.com"
body contains "Update Email"
body contains "Home"
body contains "Settings"

# Test 6: Update email should succeed
POST {{target}}/settings/email
Cookie: {{settings_session}}
Content-Type: application/x-www-form-urlencoded
[Form]
email: newemail@example.com

HTTP 200
[Asserts]
body contains "Email updated successfully"

# Test 7: Settings page should show updated email
GET {{target}}/settings
Cookie: {{settings_session}}

HTTP 200
[Asserts]
body contains "newemail@example.com"

# Test 8: Update email with invalid format should fail
POST {{target}}/settings/email
Cookie: {{settings_session}}
Content-Type: application/x-www-form-urlencoded
[Form]
email: invalid-email

HTTP 400
[Asserts]
body contains "valid email address"

# Test 9: Settings page navigation should have Settings link
GET {{target}}/
Cookie: {{settings_session}}

HTTP 200
[Asserts]
body contains "Settings"
body contains "settingsuser"
