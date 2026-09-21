import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.Reference;

public class AiCharacterFactoryEvidence extends GhidraScript {
    private void dump(long raw, DecompInterface d) throws Exception {
        Address a = toAddr(raw);
        Function f = getFunctionContaining(a);
        println(String.format("\n=== 0x%08X ===", raw));
        if (f == null) { println("NO FUNCTION"); return; }
        println("FUNCTION=" + f.getName() + " ENTRY=" + f.getEntryPoint());
        for (Reference r : getReferencesTo(f.getEntryPoint())) {
            Function caller = getFunctionContaining(r.getFromAddress());
            println("REF " + r.getFromAddress() + " " + r.getReferenceType() + " " + (caller == null ? "NOFUNC" : caller.getName()+"@"+caller.getEntryPoint()));
        }
        DecompileResults res = d.decompileFunction(f, 60, monitor);
        if (res.decompileCompleted() && res.getDecompiledFunction() != null) println(res.getDecompiledFunction().getC());
    }
    @Override public void run() throws Exception {
        DecompInterface d = new DecompInterface();
        d.openProgram(currentProgram);
        long[] targets = {0x0047E740L,0x0047E9E0L,0x0046B8DEL,0x0044AC70L};
        for (long t: targets) dump(t,d);
        d.dispose();
    }
}
