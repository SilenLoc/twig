# Test that README is displayed in repo view

# First, create the repo
POST {{target}}/init
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
