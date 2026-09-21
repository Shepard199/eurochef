import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.Reference;

public class AiMoveAnimationScalarEvidence extends GhidraScript {
    private void dump(long raw, DecompInterface d) throws Exception {
        Function f = getFunctionContaining(toAddr(raw));
        println(String.format("\n=== 0x%08X ===", raw));
        if (f == null) { println("NO FUNCTION"); return; }
        println("FUNCTION=" + f.getName() + " ENTRY=" + f.getEntryPoint());
        for (Reference r : getReferencesTo(f.getEntryPoint())) {
            Function caller = getFunctionContaining(r.getFromAddress());
            println("REF " + r.getFromAddress() + " " + r.getReferenceType() + " " +
                (caller == null ? "NOFUNC" : caller.getName() + "@" + caller.getEntryPoint()));
        }
        DecompileResults r = d.decompileFunction(f, 60, monitor);
        if (r.decompileCompleted() && r.getDecompiledFunction() != null) {
            println(r.getDecompiledFunction().getC());
        }
    }

    @Override public void run() throws Exception {
        DecompInterface d = new DecompInterface();
        d.openProgram(currentProgram);
        for (long t : new long[]{0x00454E60L, 0x004F3E8DL, 0x004F2A67L, 0x0054F8A6L}) {
            dump(t, d);
        }
        d.dispose();
    }
}
