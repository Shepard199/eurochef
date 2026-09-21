import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Instruction;
public class KnightBotUpdateListing2 extends GhidraScript {
 @Override public void run() throws Exception {
   var a=toAddr(0x00465F70L); var end=toAddr(0x00466030L);
   Instruction ins=getInstructionAt(a);
   while(ins!=null && ins.getAddress().compareTo(end)<0){println(ins.getAddress()+"  "+ins);ins=ins.getNext();}
 }
}