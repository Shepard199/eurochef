import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.mem.Memory;
import ghidra.program.model.symbol.Reference;

public class PlayToStateMarkerDerivedDescriptors extends GhidraScript {
    public void run() throws Exception {
        Memory m=currentProgram.getMemory();
        Address base=toAddr(0x005DF8D4L);
        println("=== descriptor refs to base 005DF8D4 ===");
        for(Reference r:getReferencesTo(base)){
            long from=r.getFromAddress().getOffset();
            if(from<0x005D0000L || from>0x00600000L) continue;
            println(String.format("DESC %08X",from));
            for(int off=0;off<=12;off+=4){
                int v=m.getInt(toAddr(from+off));
                println(String.format("  +%02X %08X",off,v));
            }
        }
    }
}
