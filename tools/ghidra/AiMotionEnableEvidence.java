import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.listing.InstructionIterator;
import java.util.LinkedHashSet;
import java.util.Set;

public class AiMotionEnableEvidence extends GhidraScript {
    @Override
    public void run() throws Exception {
        Set<Function> functions = new LinkedHashSet<>();
        InstructionIterator it = currentProgram.getListing().getInstructions(toAddr(0x00450000L), true);
        while (it.hasNext()) {
            Instruction ins = it.next();
            long raw = ins.getAddress().getOffset();
            if (raw >= 0x00470000L) break;
            String text = ins.toString().toLowerCase();
            if (!(text.contains("0x5f8") || text.contains("0x5dc") || text.contains("0x5e0"))) continue;
            Function f = getFunctionContaining(ins.getAddress());
            println((f == null ? "NOFUNC" : f.getName()+"@"+f.getEntryPoint()) + " " + ins.getAddress() + " " + ins);
            if (f != null) functions.add(f);
        }
        DecompInterface d = new DecompInterface();
        d.openProgram(currentProgram);
        for (Function f : functions) {
            println("\n=== " + f.getName() + " " + f.getEntryPoint() + " ===");
            DecompileResults r = d.decompileFunction(f, 60, monitor);
            if (r.decompileCompleted() && r.getDecompiledFunction() != null) println(r.getDecompiledFunction().getC());
        }
        d.dispose();
    }
}
