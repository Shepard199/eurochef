import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.FunctionManager;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.listing.InstructionIterator;
import ghidra.program.model.listing.Listing;
import ghidra.program.model.symbol.Reference;

public class Stage521Evidence extends GhidraScript {
    private void dumpFunction(long raw, int maxInstructions) throws Exception {
        Address address = toAddr(raw);
        FunctionManager fm = currentProgram.getFunctionManager();
        Listing listing = currentProgram.getListing();
        Function function = fm.getFunctionContaining(address);
        println(String.format("TARGET 0x%08X", raw));
        if (function == null) {
            disassemble(address);
            function = fm.getFunctionContaining(address);
        }
        if (function == null) {
            function = createFunction(address, null);
        }
        if (function == null) {
            println("  unable to create containing function");
            return;
        }
        println("  function=" + function.getName() + " entry=" + function.getEntryPoint());
        InstructionIterator it = listing.getInstructions(function.getBody(), true);
        int count = 0;
        while (it.hasNext() && count < maxInstructions) {
            Instruction ins = it.next();
            println("  " + ins.getAddress() + "  " + ins.toString());
            for (Reference ref : ins.getReferencesFrom()) {
                println("    REF " + ref.getReferenceType() + " -> " + ref.getToAddress());
            }
            count++;
        }
        println("  instructions_dumped=" + count);
    }

    @Override
    public void run() throws Exception {
        println("PROGRAM=" + currentProgram.getName());
        dumpFunction(0x0046C490L, 260);
        dumpFunction(0x00423DC0L, 220);
        dumpFunction(0x00509C48L, 100);
    }
}
