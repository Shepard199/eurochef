import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;

public class MineBotMotionEvidence extends GhidraScript {
    private void dump(long raw, DecompInterface d) throws Exception {
        Function f = getFunctionContaining(toAddr(raw));
        println(String.format("\n=== 0x%08X ===", raw));
        if (f == null) { println("NO FUNCTION"); return; }
        println("FUNCTION=" + f.getName() + " ENTRY=" + f.getEntryPoint());
        DecompileResults r = d.decompileFunction(f, 60, monitor);
        if (r.decompileCompleted() && r.getDecompiledFunction() != null) {
            println(r.getDecompiledFunction().getC());
        }
    }

    @Override public void run() throws Exception {
        DecompInterface d = new DecompInterface();
        d.openProgram(currentProgram);
        for (long t : new long[]{0x0045FEA0L, 0x00460340L, 0x00452800L, 0x00452E70L, 0x00452F40L, 0x00452FB0L}) {
            dump(t, d);
        }
        d.dispose();
    }
}
