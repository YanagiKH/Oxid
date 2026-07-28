fn trace(message) { print message; return message; }
fn expect(value, message) { if (value) { return value; } print message; return value; }
fn panic(message) { print message; return false; }
