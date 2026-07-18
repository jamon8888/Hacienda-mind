package hacienda-mcp.fixtures

class Greeter(val name: String) {
    fun hello(): String {
        return greet(name)
    }

    fun greet(target: String): String {
        return "Hello, $target"
    }
}
