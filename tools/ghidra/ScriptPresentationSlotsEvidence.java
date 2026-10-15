import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;

public class ScriptPresentationSlotsEvidence extends GhidraScript {
    private void decompile(long raw, DecompInterface d) throws Exception {
        Address a = toAddr(raw);
        Function f = getFunctionContaining(a);
        println(String.format("\n=== FUNC 0x%08X ===", raw));
        if (f == null) { println("NO FUNCTION"); return; }
        println("ENTRY=" + f.getEntryPoint() + " NAME=" + f.getName());
        DecompileResults r = d.decompileFunction(f, 60, monitor);
        if (r.decompileCompleted() && r.getDecompiledFunction() != null) {
            println(r.getDecompiledFunction().getC());
        }
    }

    @Override
    public void run() throws Exception {
        DecompInterface d = new DecompInterface();
        d.openProgram(currentProgram);
        decompile(0x00403B50L, d);
        decompile(0x00403AF0L, d);
        decompile(0x004AD930L, d);
        decompile(0x0049BEA0L, d);
        for (long a : new long[] {0x005DD2ACL, 0x005DD2B0L, 0x005DD2B4L, 0x005DD2B8L}) {
            println(String.format("DATA 0x%08X = 0x%08X", a, Integer.toUnsignedLong(getInt(toAddr(a)))));
        }
        d.dispose();
    }
}
