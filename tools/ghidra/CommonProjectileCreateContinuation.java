import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Instruction;

public class CommonProjectileCreateContinuation extends GhidraScript {
    @Override public void run() throws Exception {
        Address a = toAddr(0x004DFF04L);
        if (getInstructionAt(a) == null) disassemble(a);
        Instruction ins = getInstructionAt(a);
        for (int i = 0; ins != null && i < 360; i++) {
            println(ins.getAddress() + " " + ins);
            if (ins.getMnemonicString().equalsIgnoreCase("RET") && i > 20) break;
            ins = ins.getNext();
        }
    }
}
