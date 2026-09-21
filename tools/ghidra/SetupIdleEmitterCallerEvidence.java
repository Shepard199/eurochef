import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.Reference;
import java.util.LinkedHashSet;
import java.util.Set;

public class SetupIdleEmitterCallerEvidence extends GhidraScript {
    @Override public void run() throws Exception {
        DecompInterface d = new DecompInterface(); d.openProgram(currentProgram);
        Set<Function> callers = new LinkedHashSet<>();
        for (Reference r : getReferencesTo(toAddr(0x004CC480L))) {
            Function f = getFunctionContaining(r.getFromAddress());
            println("REF " + r.getFromAddress() + " " + r.getReferenceType() + " " + (f == null ? "NOFUNC" : f.getName()+"@"+f.getEntryPoint()));
            if (f != null) callers.add(f);
        }
        for (Function f : callers) {
            println("\n=== " + f.getName() + " @ " + f.getEntryPoint() + " ===");
            DecompileResults r = d.decompileFunction(f, 60, monitor);
            if (r.decompileCompleted() && r.getDecompiledFunction() != null) println(r.getDecompiledFunction().getC());
        }
        d.dispose();
    }
}
