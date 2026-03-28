
port := '8080'
test_repo := "test"
server_root := "git/srv"
export PROJECT_ROOT := "tests/{{server_root}}"


test: clean server_setup clone clean

clean:
    rm -rf git

# create a bare git repository on the server
server_setup:
    mkdir -p . {{server_root}}
    git init --bare {{server_root}}/{{test_repo}}.git

# clone the repository locally
clone:
    git clone http://localhost:{{port}}/{{test_repo}}.git git/local/{{test_repo}}
