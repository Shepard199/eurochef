import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.mem.Memory;
import ghidra.program.model.symbol.Reference;
import java.nio.charset.StandardCharsets;

public class FindFishTriggerClassEvidence extends GhidraScript {
    private void findAscii(String s) throws Exception {
        byte[] needle=s.getBytes(StandardCharsets.US_ASCII);
        Memory mem=currentProgram.getMemory();
        Address start=currentProgram.getMinAddress();
        while(true){
            Address hit=mem.findBytes(start,needle,null,true,monitor);
            if(hit==null)break;
            println("HIT "+hit+" "+s);
            for(Reference ref:getReferencesTo(hit)){
                println("  REF "+ref.getFromAddress()+" "+ref.getReferenceType());
            }
            start=hit.add(1);
        }
    }
    @Override public void run() throws Exception {
        findAscii("XTrigger_Monster_Fish");
        findAscii("Monster_Fish");
    }
}
