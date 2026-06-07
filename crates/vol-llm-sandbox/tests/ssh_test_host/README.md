# SSH Test Host

Start the test host:
`cd crates/vol-llm-sandbox/tests/ssh_test_host && docker compose up -d`

Run integration tests:
`cargo test -p vol-llm-sandbox --features ssh -- --ignored`

Stop:
`docker compose down`

First time: the host key changes on each rebuild. Remove old key:
`ssh-keygen -R '[localhost]:2222'`
