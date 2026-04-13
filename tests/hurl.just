
hurl_opts := "--variables-file variables --test"

wait-for-api:
    #!/usr/bin/env bash
    for i in {1..10}; do
        if curl -sf http://0.0.0.0:8080/up > /dev/null 2>&1; then
            echo "API is ready"
            exit 0
        fi
        echo "Waiting for API... attempt $i/10"
        sleep 1
    done
    echo "API failed to become ready"
    exit 1

# run acceptance tests against the running test stack
test *args: wait-for-api
    hurl *.hurl {{ hurl_opts }} {{ args }}
