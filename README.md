# hashfs
A file storage service

### How to use it?
Start up the service at terminal
```bash
$ RUST_LOG=info cargo run 
    Finished dev [unoptimized + debuginfo] target(s) in 0.12s
     Running `target/debug/hashfs`
[2021-01-03 16:11:02.194605 +08:00 INFO hashfs line:183] Storage init done!
[2021-01-03 16:11:02.195465 +08:00 INFO hashfs line:214] Server is running!
```
You can upload file with the curl command
```bash
$ dd if=/dev/random of=test.txt bs=1m count=5
5+0 records in
5+0 records out
5242880 bytes transferred in 0.012977 secs (404016839 bytes/sec)
$ curl http://127.0.0.1:15000 -X POST -F "file=@./test.txt"  
{"msg":"ok","result":[{"filename":"test.txt","name":"file","uri":"\"https://ycz0926.site/assets\"/\"88/77/8877209381983168103161651172620021217910114819085213602432221738154134181619433.txt\""}]}
```
