import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;

public class AiResidualCtorEvidence extends GhidraScript {
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
        decompile(0x0045AB60L, d);
        decompile(0x0045ACC0L, d);
        decompile(0x0045AD80L, d);
        decompile(0x0045AE90L, d);
        d.dispose();
    }
}
