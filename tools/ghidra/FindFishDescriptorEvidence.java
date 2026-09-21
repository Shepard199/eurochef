import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.mem.Memory;

public class FindFishDescriptorEvidence extends GhidraScript {
    @Override public void run() throws Exception {
        Memory mem=currentProgram.getMemory();
        byte[] needle=new byte[]{(byte)0xC4,(byte)0x9E,(byte)0x61,(byte)0x00};
        Address start=currentProgram.getMinAddress();
        while(true){
            Address hit=mem.findBytes(start,needle,null,true,monitor);
            if(hit==null)break;
            println("PTR_TO_FISH_NAME "+hit);
            long base=hit.getOffset()-12;
            println(String.format("  descriptor? base=0x%08X size=0x%08X ctor=0x%08X parent=0x%08X",
                base,
                Integer.toUnsignedLong(getInt(toAddr(base))),
                Integer.toUnsignedLong(getInt(toAddr(base+4))),
                Integer.toUnsignedLong(getInt(toAddr(base+8)))));
            start=hit.add(1);
        }
    }
}
