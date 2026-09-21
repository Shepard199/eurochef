import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.listing.Function;
import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;

public class AiBehaviorCreateProjectileCase extends GhidraScript {
    private void dumpEvent(int eventNumber, DecompInterface d) throws Exception {
        int index = eventNumber - 1;
        int caseIndex = getByte(toAddr(0x0044F2A0L + index)) & 0xff;
        long target = Integer.toUnsignedLong(getInt(toAddr(0x0044F288L + caseIndex * 4L)));
        println(String.format("event=0x160000%02X index=%d case=%d target=0x%08X", eventNumber, index, caseIndex, target));
        Address a=toAddr(target);
        if(getInstructionAt(a)==null) disassemble(a);
        Instruction ins=getInstructionAt(a);
        for(int i=0;ins!=null&&i<100;i++){
            println(ins.getAddress()+" "+ins);
            if(ins.getMnemonicString().equalsIgnoreCase("RET")&&i>2) break;
            ins=ins.getNext();
        }
    }

    @Override public void run() throws Exception {
        DecompInterface d=new DecompInterface(); d.openProgram(currentProgram);
        dumpEvent(0x01, d);
        dumpEvent(0x0a, d);
        dumpEvent(0x1f, d);
        d.dispose();
    }
}
