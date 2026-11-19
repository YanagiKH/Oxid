# Vectors of owned records

Store messages with their own buffers in `Vec<Message>`. In this directory:

```sh
oxid run
oxid test
oxid build --profile release
```

The program returns `42`. Its buffers are released when their owning messages leave scope.

Check `try_reserve` before inserting. `push_reserved` moves a message into reserved space without allocating; `pop_last` returns ownership of the last message. Check the length before popping. `take` moves the whole collection and leaves the original empty; `truncate` releases removed messages.

These vectors support finite records containing buffers, files, and finite enum payloads. Element indexing, element references, and nested vectors of owned records are not supported. Pop a message before accessing its fields.
