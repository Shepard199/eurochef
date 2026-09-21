import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.Instruction;

public class TransporterDetachEvidence extends GhidraScript {
    private void dump(long raw, DecompInterface decompiler) throws Exception {
        Address address = toAddr(raw);
        Function function = getFunctionContaining(address);
        println(String.format("\n=== TARGET 0x%08X ===", raw));
        if (function != null) {
            println("FUNCTION=" + function.getName() + " ENTRY=" + function.getEntryPoint());
            DecompileResults results = decompiler.decompileFunction(function, 60, monitor);
            if (results.decompileCompleted() && results.getDecompiledFunction() != null) {
                println(results.getDecompiledFunction().getC());
                return;
            }
        }
        Instruction instruction = getInstructionAt(address);
        for (int i = 0; instruction != null && i < 80; i++) {
            println(instruction.getAddress() + " " + instruction);
            if (instruction.getMnemonicString().equalsIgnoreCase("RET")) {
                break;
            }
            instruction = instruction.getNext();
        }
    }

    @Override
    public void run() throws Exception {
        println("PROGRAM=" + currentProgram.getName());
        DecompInterface decompiler = new DecompInterface();
        decompiler.openProgram(currentProgram);
        dump(0x004046E0L, decompiler);
        dump(0x004E8050L, decompiler);
        decompiler.dispose();
    }
}
