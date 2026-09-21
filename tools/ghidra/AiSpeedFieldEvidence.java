import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.listing.InstructionIterator;
import java.util.LinkedHashSet;
import java.util.Set;

public class AiSpeedFieldEvidence extends GhidraScript {
    @Override
    public void run() throws Exception {
        Set<Function> functions = new LinkedHashSet<>();
        InstructionIterator it = currentProgram.getListing().getInstructions(true);
        while (it.hasNext()) {
            Instruction ins = it.next();
            String text = ins.toString().toLowerCase();
            if (!(text.contains("0x5dc") || text.contains("0x5e0"))) continue;
            Function f = getFunctionContaining(ins.getAddress());
            println((f == null ? "NOFUNC" : f.getName()) + " " + ins.getAddress() + " " + ins);
            if (f != null) functions.add(f);
        }

        DecompInterface decompiler = new DecompInterface();
        decompiler.openProgram(currentProgram);
        for (Function f : functions) {
            println("\n=== " + f.getName() + " " + f.getEntryPoint() + " ===");
            DecompileResults results = decompiler.decompileFunction(f, 60, monitor);
            if (results.decompileCompleted() && results.getDecompiledFunction() != null) {
                println(results.getDecompiledFunction().getC());
            }
        }
        decompiler.dispose();
    }
}
