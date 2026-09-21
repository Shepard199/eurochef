import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.symbol.ReferenceIterator;
import java.util.LinkedHashSet;
import java.util.Set;

public class PiranhaTestAnimFactoryEvidence extends GhidraScript {
    private void decompile(Function f, DecompInterface d) throws Exception {
        if (f == null) return;
        println("\n=== " + f.getEntryPoint() + " " + f.getName() + " ===");
        DecompileResults r = d.decompileFunction(f, 60, monitor);
        if (r.decompileCompleted() && r.getDecompiledFunction() != null) {
            println(r.getDecompiledFunction().getC());
        }
    }

    private void refs(long raw, DecompInterface d) throws Exception {
        Address a = toAddr(raw);
        println(String.format("\n=== REFS TO 0x%08X ===", raw));
        Set<Function> fs = new LinkedHashSet<>();
        ReferenceIterator it = currentProgram.getReferenceManager().getReferencesTo(a);
        while (it.hasNext()) {
            Reference ref = it.next();
            println(ref.getFromAddress() + " " + ref.getReferenceType());
            Function f = getFunctionContaining(ref.getFromAddress());
            if (f != null) fs.add(f);
        }
        for (Function f : fs) decompile(f, d);
    }

    @Override public void run() throws Exception {
        DecompInterface d = new DecompInterface();
        d.openProgram(currentProgram);
        refs(0x005E28ECL, d); // EM07 descriptor
        refs(0x005E28FCL, d); // TestAnimBot descriptor
        decompile(getFunctionAt(toAddr(0x0047EA70L)), d);
        d.dispose();
    }
}
