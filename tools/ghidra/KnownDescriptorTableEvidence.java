import ghidra.app.script.GhidraScript;
public class KnownDescriptorTableEvidence extends GhidraScript {
    private long ptr(long a) throws Exception { return Integer.toUnsignedLong(getInt(toAddr(a))); }
    private void dump(long base, String name) throws Exception {
        println("\n=== " + name + String.format(" 0x%08X ===", base));
        for (int off = 0; off <= 0x1c; off += 4) {
            println(String.format("off=%d  %08X", off, ptr(base + off)));
        }
    }
    @Override public void run() throws Exception {
        dump(0x005E262CL, "DogBot descriptor");
        dump(0x005E272CL, "SpinTop descriptor");
        dump(0x005E28ECL, "EM07 descriptor");
        dump(0x005E28FCL, "TestAnim descriptor");
    }
}
